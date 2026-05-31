use lessonforge_core::ids::{
    ActorId, LeaseId, PlanningTaskId, RequestId, RequestModerationTaskId, ReviewTaskId,
};
use lessonforge_core::moderation::ModerationReportSubmission;
use lessonforge_core::planning::PlanningTaskRecord;
use lessonforge_core::request::{
    IntakeContext, ModerationApplicationOutcome, RequestIntakeOutcome, RequestIntakePayload,
    RequestModerationContext, RequestWorkflowError, StoredRequest, accept_request_intake,
    apply_moderation_report,
};
use lessonforge_core::review::{
    FindingSeverity, FindingType, ReviewClaimContext, ReviewContext, ReviewOutcome,
    ReviewPolicyError, ReviewSubmission, ReviewSubmissionResult, ReviewTaskRecord, ReviewType,
    ReviewerProfile, SourceActorLineage, create_review_task, submit_review, validate_review_claim,
};
use lessonforge_core::state::{
    ArtifactState, Lease, ProposedTaskGraphState, RequestState, ReviewTaskState,
};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

#[derive(Debug, Default)]
pub struct DeterministicWorkflow {
    request: Option<StoredRequest>,
    moderation_task_id: Option<RequestModerationTaskId>,
    moderation_claim: Option<ModerationClaim>,
    moderation_outcome: Option<StoredModerationOutcome>,
    planning_tasks: Vec<PlanningTaskRecord>,
    review_task: Option<ReviewTaskRecord>,
    review_claim: Option<ReviewClaim>,
    review_claim_replays: Vec<StoredReviewClaim>,
    review_submission: Option<StoredReviewSubmission>,
    review_gate_state: Option<ReviewGateState>,
    artifact_state: Option<ArtifactState>,
    now: u64,
}

impl DeterministicWorkflow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submit_request(
        &mut self,
        payload: RequestIntakePayload,
        context: IntakeContext,
    ) -> Result<RequestIntakeOutcome, RequestWorkflowError> {
        if self.request.is_some() {
            return Err(RequestWorkflowError::Conflict {
                reason: "workflow_request_already_active",
            });
        }
        let outcome = accept_request_intake(payload, context)?;
        self.moderation_task_id = Some(outcome.moderation_task.task_id.clone());
        self.request = Some(outcome.request.clone());
        Ok(outcome)
    }

    pub fn claim_request_moderation_task(
        &mut self,
        task_id: RequestModerationTaskId,
        lease_id: LeaseId,
        actor_id: ActorId,
        claim_token: &str,
    ) -> Result<(), RequestWorkflowError> {
        let Some(request) = &self.request else {
            return Err(RequestWorkflowError::ModerationRejected {
                reason: "request_not_found",
            });
        };
        let Some(expected_task_id) = &self.moderation_task_id else {
            return Err(RequestWorkflowError::ModerationRejected {
                reason: "moderation_task_not_found",
            });
        };
        if expected_task_id != &task_id {
            return Err(RequestWorkflowError::ModerationRejected {
                reason: "moderation_task_mismatch",
            });
        }
        if self
            .moderation_claim
            .as_ref()
            .is_some_and(|claim| claim.active)
        {
            return Err(RequestWorkflowError::ModerationRejected {
                reason: "moderation_lease_already_active",
            });
        }
        self.moderation_claim = Some(ModerationClaim {
            request_id: request.request_id.clone(),
            task_id,
            lease_id,
            actor_id,
            claim_token_hash: Lease::claim_token_hash(claim_token),
            active: true,
        });
        Ok(())
    }

    pub fn submit_moderation_report(
        &mut self,
        report: ModerationReportSubmission,
    ) -> Result<ModerationApplicationOutcome, RequestWorkflowError> {
        let Some(request) = &self.request else {
            return Err(RequestWorkflowError::ModerationRejected {
                reason: "request_not_found",
            });
        };
        if let Some(stored) = &self.moderation_outcome
            && stored.report_id == report.request_moderation_report_id
            && stored.task_id == report.request_moderation_task_id
            && stored.lease_id == report.lease_id
        {
            return Ok(stored.outcome.clone());
        }
        let Some(claim) = &self.moderation_claim else {
            return Err(RequestWorkflowError::ModerationRejected {
                reason: "moderation_lease_not_found",
            });
        };
        if claim.request_id != report.request_id
            || claim.task_id != report.request_moderation_task_id
            || claim.lease_id != report.lease_id
        {
            return Err(RequestWorkflowError::ModerationRejected {
                reason: "moderation_lease_mismatch",
            });
        }
        let context = RequestModerationContext {
            request_id: claim.request_id.clone(),
            moderation_task_id: claim.task_id.clone(),
            planning_task_id: PlanningTaskId::try_from(format!(
                "ptask_{}",
                claim.request_id.as_str().strip_prefix("req_").ok_or(
                    RequestWorkflowError::ModerationRejected {
                        reason: "planning_task_id_derivation_failed",
                    }
                )?
            ))
            .map_err(|_| RequestWorkflowError::ModerationRejected {
                reason: "planning_task_id_derivation_failed",
            })?,
            moderator_actor_id: claim.actor_id.clone(),
            lease_id: claim.lease_id.clone(),
            claim_token_hash: claim.claim_token_hash.clone(),
            scope_id: request.scope_id.clone(),
            lease_active: claim.active,
            actor_scope_matches: true,
            actor_can_moderate: true,
        };
        let report_id = report.request_moderation_report_id.clone();
        let task_id = report.request_moderation_task_id.clone();
        let lease_id = report.lease_id.clone();
        let outcome = apply_moderation_report(request, context, report)?;
        let mut updated_request = request.clone();
        updated_request.state = outcome.request_state;
        self.request = Some(updated_request);
        if let Some(planning_task) = &outcome.planning_task {
            self.planning_tasks.push(planning_task.clone());
        }
        self.moderation_outcome = Some(StoredModerationOutcome {
            report_id,
            task_id,
            lease_id,
            outcome: outcome.clone(),
        });
        Ok(outcome)
    }

    pub fn planning_task_count(&self) -> usize {
        self.planning_tasks.len()
    }

    pub fn set_now(&mut self, now: u64) {
        self.now = now;
    }

    pub fn open_review_task(
        &mut self,
        context: ReviewContext,
    ) -> Result<ReviewTaskRecord, ReviewPolicyError> {
        if self.review_task.is_some()
            || self
                .artifact_state
                .is_some_and(|state| state != ArtifactState::MachineValidated)
        {
            return Err(ReviewPolicyError::ReviewSourceStateNotEligible);
        }
        let gate_state = ReviewGateState::from_open_context(&context);
        let task = create_review_task(context)?;
        self.artifact_state = Some(ArtifactState::ReviewRequested);
        self.review_gate_state = Some(gate_state);
        self.review_task = Some(task.clone());
        Ok(task)
    }

    pub fn set_review_gate_state(&mut self, context: ReviewContext) {
        self.artifact_state = Some(context.artifact_state);
        self.review_gate_state = Some(ReviewGateState {
            artifact_state: context.artifact_state,
            request_state: context.request_state,
            proposal_state: context.proposal_state,
            trusted_validation_passed: context.trusted_validation_passed,
            open_blocking_findings_elsewhere: context.open_blocking_validation_findings,
        });
    }

    pub fn claim_review_task(
        &mut self,
        review_task_id: ReviewTaskId,
        lease_id: LeaseId,
        idempotency_key: &str,
        reviewer: ReviewerProfile,
        source_lineage: SourceActorLineage,
    ) -> Result<ReviewClaimResult, ReviewPolicyError> {
        if idempotency_key.is_empty() {
            return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
        }
        validate_idempotency_key(idempotency_key)?;
        self.expire_review_claim_if_needed();
        if let Some(replay) = self.review_claim_replays.iter().find(|replay| {
            replay.task_id == review_task_id
                && replay.idempotency_key == idempotency_key
                && replay.reviewer.reviewer_actor_id == reviewer.reviewer_actor_id
        }) {
            if replay.task_id != review_task_id
                || replay.lease_id != lease_id
                || replay.reviewer != reviewer
                || replay.source_lineage != source_lineage
            {
                return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
            }
            return Ok(ReviewClaimResult {
                claim_token_returned: false,
                claim_token: None,
                ..replay.result.clone()
            });
        }
        let Some(task) = &self.review_task else {
            return Err(ReviewPolicyError::ReviewSourceStateNotEligible);
        };
        if task.review_task_id != review_task_id {
            return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
        }
        if task.state != ReviewTaskState::Open
            || self.review_claim.as_ref().is_some_and(|claim| claim.active)
        {
            return Err(ReviewPolicyError::ReviewLeaseNotActive);
        }
        let gate = self.current_review_claim_gate_context()?;
        let probe = ReviewClaimContext {
            reviewer: reviewer.clone(),
            source_lineage: source_lineage.clone(),
            lease_active: true,
            artifact_state: gate.artifact_state,
            request_state: gate.request_state,
            proposal_state: gate.proposal_state,
            now: self.now,
            trusted_validation_passed: gate.trusted_validation_passed,
            open_blocking_findings_elsewhere: gate.open_blocking_findings_elsewhere,
        };
        validate_review_claim(task, &probe)?;
        let result = ReviewClaimResult {
            review_task_id: review_task_id.clone(),
            lease_id: lease_id.clone(),
            review_task_state: ReviewTaskState::Claimed,
            claim_token_returned: true,
            claim_token: Some(derive_review_claim_token(&lease_id, idempotency_key)),
            expires_at: self.now.saturating_add(REVIEW_LEASE_TTL_SECONDS),
        };
        self.review_claim = Some(ReviewClaim {
            task_id: review_task_id.clone(),
            lease_id: lease_id.clone(),
            idempotency_key: idempotency_key.to_owned(),
            reviewer: reviewer.clone(),
            source_lineage: source_lineage.clone(),
            active: true,
            expires_at: result.expires_at,
        });
        self.review_claim_replays.push(StoredReviewClaim {
            task_id: review_task_id,
            lease_id,
            idempotency_key: idempotency_key.to_owned(),
            reviewer,
            source_lineage,
            result: ReviewClaimResult {
                claim_token_returned: false,
                claim_token: None,
                ..result.clone()
            },
        });
        self.review_task = Some(ReviewTaskRecord {
            state: ReviewTaskState::Claimed,
            ..task.clone()
        });
        Ok(result)
    }

    pub fn submit_human_review(
        &mut self,
        review_task_id: ReviewTaskId,
        lease_id: LeaseId,
        claim_token: &str,
        idempotency_key: &str,
        submission: ReviewSubmission,
    ) -> Result<ReviewSubmissionResult, ReviewPolicyError> {
        if idempotency_key.is_empty() {
            return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
        }
        validate_idempotency_key(idempotency_key)?;
        let submission_digest = review_submission_digest(&submission);
        if let Some(stored) = &self.review_submission
            && stored.review_task_id == review_task_id
            && stored.lease_id == lease_id
            && stored.idempotency_key == idempotency_key
        {
            if !review_claim_token_matches(
                &stored.lease_id,
                &stored.claim_idempotency_key,
                claim_token,
            ) {
                return Err(ReviewPolicyError::ReviewLeaseNotActive);
            }
            if stored.submission_digest != submission_digest {
                return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
            }
            return Ok(stored.result.clone());
        }
        self.expire_review_claim_if_needed();
        let Some(task) = &self.review_task else {
            return Err(ReviewPolicyError::ReviewSourceStateNotEligible);
        };
        let Some(claim) = &self.review_claim else {
            return Err(ReviewPolicyError::ReviewLeaseNotActive);
        };
        if claim.is_expired(self.now) {
            return Err(ReviewPolicyError::ReviewLeaseNotActive);
        }
        if claim.task_id != review_task_id
            || claim.lease_id != lease_id
            || !review_claim_token_matches(&claim.lease_id, &claim.idempotency_key, claim_token)
        {
            return Err(ReviewPolicyError::ReviewLeaseNotActive);
        }
        let reviewer = claim.reviewer.clone();
        let source_lineage = claim.source_lineage.clone();
        let claim_active = claim.active;
        let claim_idempotency_key = claim.idempotency_key.clone();
        let gate = self.current_review_claim_gate_context()?;
        let result = submit_review(
            task.clone(),
            ReviewClaimContext {
                reviewer,
                source_lineage,
                lease_active: claim_active,
                artifact_state: gate.artifact_state,
                request_state: gate.request_state,
                proposal_state: gate.proposal_state,
                now: self.now,
                trusted_validation_passed: gate.trusted_validation_passed,
                open_blocking_findings_elsewhere: gate.open_blocking_findings_elsewhere,
            },
            submission.clone(),
        )?;
        self.artifact_state = Some(result.artifact_state);
        self.review_task = Some(ReviewTaskRecord {
            state: result.system_completed_review_task_state,
            ..task.clone()
        });
        if let Some(claim) = &mut self.review_claim {
            claim.active = false;
        }
        self.review_submission = Some(StoredReviewSubmission {
            review_task_id,
            lease_id,
            claim_idempotency_key,
            idempotency_key: idempotency_key.to_owned(),
            submission_digest,
            result: result.clone(),
        });
        Ok(result)
    }

    pub fn current_artifact_state(&self) -> Option<ArtifactState> {
        self.artifact_state
    }

    fn current_review_claim_gate_context(
        &self,
    ) -> Result<ReviewClaimContextTail, ReviewPolicyError> {
        let Some(gate_state) = &self.review_gate_state else {
            return Err(ReviewPolicyError::ReviewSourceStateNotEligible);
        };
        Ok(ReviewClaimContextTail {
            artifact_state: self.artifact_state.unwrap_or(gate_state.artifact_state),
            request_state: gate_state.request_state,
            proposal_state: gate_state.proposal_state,
            trusted_validation_passed: gate_state.trusted_validation_passed,
            open_blocking_findings_elsewhere: gate_state.open_blocking_findings_elsewhere,
        })
    }

    fn expire_review_claim_if_needed(&mut self) {
        let expired = self
            .review_claim
            .as_ref()
            .is_some_and(|claim| claim.active && claim.is_expired(self.now));
        if !expired {
            return;
        }
        if let Some(claim) = &mut self.review_claim {
            claim.active = false;
        }
        if let Some(task) = &self.review_task
            && task.state == ReviewTaskState::Claimed
        {
            self.review_task = Some(ReviewTaskRecord {
                state: ReviewTaskState::Open,
                ..task.clone()
            });
        }
    }
}

const REVIEW_LEASE_TTL_SECONDS: u64 = 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewGateState {
    artifact_state: ArtifactState,
    request_state: RequestState,
    proposal_state: ProposedTaskGraphState,
    trusted_validation_passed: bool,
    open_blocking_findings_elsewhere: bool,
}

impl ReviewGateState {
    fn from_open_context(context: &ReviewContext) -> Self {
        Self {
            artifact_state: ArtifactState::ReviewRequested,
            request_state: context.request_state,
            proposal_state: context.proposal_state,
            trusted_validation_passed: context.trusted_validation_passed,
            open_blocking_findings_elsewhere: context.open_blocking_validation_findings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReviewClaimContextTail {
    artifact_state: ArtifactState,
    request_state: RequestState,
    proposal_state: ProposedTaskGraphState,
    trusted_validation_passed: bool,
    open_blocking_findings_elsewhere: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewClaimResult {
    pub review_task_id: ReviewTaskId,
    pub lease_id: LeaseId,
    pub review_task_state: ReviewTaskState,
    pub claim_token_returned: bool,
    pub claim_token: Option<String>,
    pub expires_at: u64,
}

#[derive(Debug, Clone)]
struct ModerationClaim {
    request_id: RequestId,
    task_id: RequestModerationTaskId,
    lease_id: LeaseId,
    actor_id: ActorId,
    claim_token_hash: String,
    active: bool,
}

#[derive(Debug, Clone)]
struct StoredModerationOutcome {
    report_id: lessonforge_core::ids::RequestModerationReportId,
    task_id: RequestModerationTaskId,
    lease_id: LeaseId,
    outcome: ModerationApplicationOutcome,
}

#[derive(Debug, Clone)]
struct ReviewClaim {
    task_id: ReviewTaskId,
    lease_id: LeaseId,
    idempotency_key: String,
    reviewer: ReviewerProfile,
    source_lineage: SourceActorLineage,
    active: bool,
    expires_at: u64,
}

#[derive(Debug, Clone)]
struct StoredReviewClaim {
    task_id: ReviewTaskId,
    lease_id: LeaseId,
    idempotency_key: String,
    reviewer: ReviewerProfile,
    source_lineage: SourceActorLineage,
    result: ReviewClaimResult,
}

impl ReviewClaim {
    fn is_expired(&self, now: u64) -> bool {
        now >= self.expires_at
    }
}

#[derive(Debug, Clone)]
struct StoredReviewSubmission {
    review_task_id: ReviewTaskId,
    lease_id: LeaseId,
    claim_idempotency_key: String,
    idempotency_key: String,
    submission_digest: String,
    result: ReviewSubmissionResult,
}

fn derive_review_claim_token(lease_id: &LeaseId, claim_idempotency_key: &str) -> String {
    digest_hex(format!(
        "lessonforge-review-claim-token-v2\0{}\0{}\0{}",
        review_claim_verifier_salt(),
        lease_id.as_str(),
        claim_idempotency_key
    ))
}

fn review_claim_token_matches(
    lease_id: &LeaseId,
    claim_idempotency_key: &str,
    claim_token: &str,
) -> bool {
    if claim_token.len() != 64 {
        return false;
    }
    constant_time_eq(
        derive_review_claim_token(lease_id, claim_idempotency_key).as_bytes(),
        claim_token.as_bytes(),
    )
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0;
    for (left_byte, right_byte) in left.iter().zip(right) {
        diff |= usize::from(left_byte ^ right_byte);
    }
    diff == 0
}

fn review_claim_verifier_salt() -> &'static str {
    static SALT: OnceLock<String> = OnceLock::new();
    SALT.get_or_init(|| {
        let mut bytes = [0u8; 32];
        if getrandom::fill(&mut bytes).is_err() {
            std::process::abort();
        }
        let mut output = String::from("review-claim-verifier-v2:");
        for byte in bytes {
            output.push(hex_char(byte >> 4));
            output.push(hex_char(byte & 0x0f));
        }
        output
    })
}

fn validate_idempotency_key(value: &str) -> Result<(), ReviewPolicyError> {
    let lower = value.to_ascii_lowercase();
    let valid = (8..=128).contains(&value.len())
        && value
            .chars()
            .all(|character| character.is_ascii_graphic() && !character.is_ascii_whitespace())
        && !lower.contains("http://")
        && !lower.contains("https://")
        && !lower.contains("sk-")
        && !lower.contains("api_key")
        && !lower.contains("cookie")
        && !lower.contains("credential")
        && !lower.contains("oauth")
        && !lower.contains("password")
        && !lower.contains("provider")
        && !lower.contains("secret")
        && !lower.contains("token");
    if valid {
        Ok(())
    } else {
        Err(ReviewPolicyError::ReviewSubmissionLineageMismatch)
    }
}

fn review_submission_digest(submission: &ReviewSubmission) -> String {
    let mut canonical = String::new();
    canonical.push_str("review-submission-v2");
    let mut review_types = submission
        .review_types
        .iter()
        .map(|value| review_type_name(*value))
        .collect::<Vec<_>>();
    review_types.sort_unstable();
    append_len(
        &mut canonical,
        "review_type_count",
        &review_types.len().to_string(),
    );
    for review_type in review_types {
        append_len(&mut canonical, "review_type", review_type);
    }
    append_len(
        &mut canonical,
        "outcome",
        review_outcome_name(submission.outcome),
    );
    append_len(
        &mut canonical,
        "recommended_next_state",
        submission
            .recommended_next_state
            .as_deref()
            .unwrap_or("<none>"),
    );
    append_len(
        &mut canonical,
        "reviewer_id",
        submission
            .reviewer_id
            .as_ref()
            .map(ActorId::as_str)
            .unwrap_or("<none>"),
    );
    append_len(
        &mut canonical,
        "submission_authority_count",
        &submission.submitted_authority_fields.len().to_string(),
    );
    for field in &submission.submitted_authority_fields {
        append_len(&mut canonical, "submission_authority", field);
    }
    append_len(
        &mut canonical,
        "finding_count",
        &submission.findings.len().to_string(),
    );
    for finding in &submission.findings {
        append_len(
            &mut canonical,
            "finding_severity",
            finding_severity_name(finding.severity),
        );
        append_len(
            &mut canonical,
            "finding_type",
            finding_type_name(finding.finding_type),
        );
        append_len(&mut canonical, "finding_location", &finding.safe_location);
        append_len(&mut canonical, "finding_message", &finding.safe_message);
        append_len(
            &mut canonical,
            "finding_submitted_blocking",
            match finding.submitted_blocking {
                Some(true) => "blocking_true",
                Some(false) => "blocking_false",
                None => "blocking_none",
            },
        );
        append_len(
            &mut canonical,
            "finding_authority_count",
            &finding.submitted_authority_fields.len().to_string(),
        );
        for field in &finding.submitted_authority_fields {
            append_len(&mut canonical, "finding_authority", field);
        }
    }
    digest_hex(canonical)
}

fn append_len(canonical: &mut String, label: &str, value: &str) {
    canonical.push('\n');
    canonical.push_str(label);
    canonical.push(':');
    canonical.push_str(&value.len().to_string());
    canonical.push(':');
    canonical.push_str(value);
}

fn digest_hex(input: String) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(hex_char(byte >> 4));
        output.push(hex_char(byte & 0x0f));
    }
    output
}

fn hex_char(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => '0',
    }
}

fn review_type_name(value: ReviewType) -> &'static str {
    match value {
        ReviewType::SubjectCorrectness => "subject_correctness",
        ReviewType::Pedagogy => "pedagogy",
    }
}

fn review_outcome_name(value: ReviewOutcome) -> &'static str {
    match value {
        ReviewOutcome::ApprovedForPeerReviewed => "approved_for_peer_reviewed",
        ReviewOutcome::ChangesRequested => "changes_requested",
        ReviewOutcome::RejectedForUse => "rejected_for_use",
        ReviewOutcome::NeedsSubjectMatterFix => "needs_subject_matter_fix",
    }
}

fn finding_severity_name(value: FindingSeverity) -> &'static str {
    match value {
        FindingSeverity::Critical => "critical",
        FindingSeverity::Major => "major",
        FindingSeverity::Minor => "minor",
        FindingSeverity::Note => "note",
    }
}

fn finding_type_name(value: FindingType) -> &'static str {
    match value {
        FindingType::PhysicsError => "physics_error",
        FindingType::UnsafeInstruction => "unsafe_instruction",
        FindingType::MissingAnswerKey => "missing_answer_key",
        FindingType::CheckerMismatch => "checker_mismatch",
        FindingType::LicenseIssue => "license_issue",
        FindingType::PiiOrSecretLeak => "pii_or_secret_leak",
        FindingType::PedagogyIssue => "pedagogy_issue",
        FindingType::AccessibilityNote => "accessibility_note",
        FindingType::FormattingIssue => "formatting_issue",
        FindingType::OtherSafe => "other_safe",
    }
}
