use crate::ids::{ActorId, ArtifactId, FindingId, RequestId, ReviewId, ReviewTaskId, WorkPacketId};
use crate::state::{
    ActorCapability, ActorStatus, ActorType, ArtifactState, ProposedTaskGraphState, RequestState,
    ReviewTaskState, TrustLevel,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewType {
    SubjectCorrectness,
    Pedagogy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicArtifactLabel {
    MachineValidated,
    PeerReviewed,
    Deprecated,
}

impl PublicArtifactLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MachineValidated => "machine_validated",
            Self::PeerReviewed => "peer_reviewed",
            Self::Deprecated => "deprecated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPublicationInput {
    pub state: ArtifactState,
    pub visibility: ArtifactVisibility,
    pub previously_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewContext {
    pub review_task_id: ReviewTaskId,
    pub review_work_packet_id: WorkPacketId,
    pub artifact_id: ArtifactId,
    pub request_id: RequestId,
    pub scope_id: String,
    pub artifact_state: ArtifactState,
    pub request_state: RequestState,
    pub proposal_state: ProposedTaskGraphState,
    pub required_subject: String,
    pub required_age_range: String,
    pub trusted_validation_passed: bool,
    pub human_review_work_packet_proof: HumanReviewWorkPacketProof,
    pub open_blocking_validation_findings: bool,
    pub existing_active_review_task: bool,
    pub visibility: ArtifactVisibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanReviewWorkPacketProof {
    pub work_packet_exists: bool,
    pub depends_on_validation_work_packet: bool,
    pub request_lineage_matches: bool,
    pub artifact_lineage_matches: bool,
    pub scope_lineage_matches: bool,
}

impl HumanReviewWorkPacketProof {
    pub fn valid() -> Self {
        Self {
            work_packet_exists: true,
            depends_on_validation_work_packet: true,
            request_lineage_matches: true,
            artifact_lineage_matches: true,
            scope_lineage_matches: true,
        }
    }

    fn is_valid(&self) -> bool {
        self.work_packet_exists
            && self.depends_on_validation_work_packet
            && self.request_lineage_matches
            && self.artifact_lineage_matches
            && self.scope_lineage_matches
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTaskRecord {
    pub review_task_id: ReviewTaskId,
    pub review_work_packet_id: WorkPacketId,
    pub artifact_id: ArtifactId,
    pub request_id: RequestId,
    pub scope_id: String,
    pub review_types: Vec<ReviewType>,
    pub required_subject: String,
    pub required_age_range: String,
    pub required_reviewer_trust_level: TrustLevel,
    pub state: ReviewTaskState,
    pub visibility: ArtifactVisibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerProfile {
    pub reviewer_actor_id: ActorId,
    pub actor_type: ActorType,
    pub status: ActorStatus,
    pub scope_id: String,
    pub operator_account_id: String,
    pub conflict_group_id: String,
    pub independence_verified: bool,
    pub review_capabilities: Vec<ActorCapability>,
    pub trusted_subjects: Vec<String>,
    pub trusted_age_ranges: Vec<String>,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceActorLineage {
    pub planner_actor_id: ActorId,
    pub verifier_actor_id: ActorId,
    pub generator_actor_id: ActorId,
    pub accepted_output_actors: Vec<SourceOutputActor>,
    pub planner_operator_account_id: String,
    pub verifier_operator_account_id: String,
    pub generator_operator_account_id: String,
    pub planner_conflict_group_id: String,
    pub verifier_conflict_group_id: String,
    pub generator_conflict_group_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceOutputActor {
    pub actor_id: ActorId,
    pub operator_account_id: String,
    pub conflict_group_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewClaimContext {
    pub reviewer: ReviewerProfile,
    pub source_lineage: SourceActorLineage,
    pub lease_active: bool,
    pub artifact_state: ArtifactState,
    pub request_state: RequestState,
    pub proposal_state: ProposedTaskGraphState,
    pub now: u64,
    pub trusted_validation_passed: bool,
    pub open_blocking_findings_elsewhere: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutcome {
    ApprovedForPeerReviewed,
    ChangesRequested,
    RejectedForUse,
    NeedsSubjectMatterFix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingSeverity {
    Critical,
    Major,
    Minor,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingType {
    PhysicsError,
    UnsafeInstruction,
    MissingAnswerKey,
    CheckerMismatch,
    LicenseIssue,
    PiiOrSecretLeak,
    PedagogyIssue,
    AccessibilityNote,
    FormattingIssue,
    OtherSafe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingInput {
    pub severity: FindingSeverity,
    pub finding_type: FindingType,
    pub safe_location: String,
    pub safe_message: String,
    pub submitted_blocking: Option<bool>,
    pub submitted_authority_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewSubmission {
    pub review_types: Vec<ReviewType>,
    pub outcome: ReviewOutcome,
    pub findings: Vec<FindingInput>,
    pub recommended_next_state: Option<String>,
    pub reviewer_id: Option<ActorId>,
    pub submitted_authority_fields: Vec<String>,
}

impl ReviewSubmission {
    pub fn approved_no_findings() -> Self {
        Self {
            review_types: vec![ReviewType::SubjectCorrectness, ReviewType::Pedagogy],
            outcome: ReviewOutcome::ApprovedForPeerReviewed,
            findings: Vec::new(),
            recommended_next_state: None,
            reviewer_id: None,
            submitted_authority_fields: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRecord {
    pub schema_version: &'static str,
    pub review_id: ReviewId,
    pub review_task_id: ReviewTaskId,
    pub review_work_packet_id: WorkPacketId,
    pub request_id: RequestId,
    pub scope_id: String,
    pub artifact_id: ArtifactId,
    pub reviewer_actor_id: ActorId,
    pub source_lineage: SourceActorLineage,
    pub review_types: Vec<ReviewType>,
    pub outcome: ReviewOutcome,
    pub recommended_next_state: Option<String>,
    pub created_at: u64,
    pub state: ReviewRecordState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRecordState {
    Accepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingParentType {
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRecord {
    pub finding_id: FindingId,
    pub parent_type: FindingParentType,
    pub parent_id: ReviewId,
    pub created_by_actor_id: ActorId,
    pub created_at: u64,
    pub severity: FindingSeverity,
    pub finding_type: FindingType,
    pub safe_location: String,
    pub safe_message: String,
    pub blocking: bool,
    pub state: FindingState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingState {
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewSubmissionResult {
    pub review: ReviewRecord,
    pub findings: Vec<FindingRecord>,
    pub review_task_state: ReviewTaskState,
    pub system_completed_review_task_state: ReviewTaskState,
    pub artifact_state: ArtifactState,
    pub public_label: Option<PublicArtifactLabel>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReviewPolicyError {
    #[error("review source state is not eligible")]
    ReviewSourceStateNotEligible,
    #[error("review conflict")]
    ReviewConflict,
    #[error("review lease is not active")]
    ReviewLeaseNotActive,
    #[error("review submission lineage mismatch")]
    ReviewSubmissionLineageMismatch,
    #[error("review submitted authority field")]
    ReviewSubmittedAuthorityField,
    #[error("submitted finding authority field")]
    SubmittedFindingAuthorityField,
    #[error("invalid finding")]
    InvalidFinding,
    #[error("unsafe submitted review text")]
    UnsafeSubmittedReviewText,
}

impl ReviewPolicyError {
    pub fn safe_code(&self) -> &'static str {
        match self {
            Self::ReviewSourceStateNotEligible => "review_source_state_not_eligible",
            Self::ReviewConflict => "review_conflict",
            Self::ReviewLeaseNotActive => "review_lease_not_active",
            Self::ReviewSubmissionLineageMismatch => "review_submission_lineage_mismatch",
            Self::ReviewSubmittedAuthorityField => "review_submitted_authority_field",
            Self::SubmittedFindingAuthorityField => "submitted_finding_authority_field",
            Self::InvalidFinding => "invalid_finding",
            Self::UnsafeSubmittedReviewText => "unsafe_submitted_review_text",
        }
    }
}

pub fn create_review_task(context: ReviewContext) -> Result<ReviewTaskRecord, ReviewPolicyError> {
    if context.artifact_state != ArtifactState::MachineValidated
        || !source_context_can_promote(context.request_state, context.proposal_state)
        || !context.trusted_validation_passed
        || !context.human_review_work_packet_proof.is_valid()
        || context.open_blocking_validation_findings
        || context.existing_active_review_task
    {
        return Err(ReviewPolicyError::ReviewSourceStateNotEligible);
    }
    Ok(ReviewTaskRecord {
        review_task_id: context.review_task_id,
        review_work_packet_id: context.review_work_packet_id,
        artifact_id: context.artifact_id,
        request_id: context.request_id,
        scope_id: context.scope_id,
        review_types: vec![ReviewType::SubjectCorrectness, ReviewType::Pedagogy],
        required_subject: context.required_subject,
        required_age_range: context.required_age_range,
        required_reviewer_trust_level: TrustLevel::ReviewerCandidate,
        state: ReviewTaskState::Open,
        visibility: context.visibility,
    })
}

pub fn submit_review(
    task: ReviewTaskRecord,
    context: ReviewClaimContext,
    submission: ReviewSubmission,
) -> Result<ReviewSubmissionResult, ReviewPolicyError> {
    if !context.lease_active {
        return Err(ReviewPolicyError::ReviewLeaseNotActive);
    }
    validate_reviewer(&task, &context.reviewer, &context.source_lineage)?;
    validate_submission(&task, &context.reviewer, &submission)?;
    validate_source_context(&context)?;
    let review_id = review_id_for_task(&task.review_task_id)?;
    let findings = normalize_findings(
        &task.review_task_id,
        &review_id,
        &context.reviewer.reviewer_actor_id,
        context.now,
        &submission.findings,
    )?;
    let has_blocking_findings =
        context.open_blocking_findings_elsewhere || findings.iter().any(|finding| finding.blocking);
    let promotes = context.trusted_validation_passed
        && submission.outcome == ReviewOutcome::ApprovedForPeerReviewed
        && exact_review_types(&submission.review_types)
        && !has_blocking_findings;
    let artifact_state = if promotes {
        ArtifactState::PeerReviewed
    } else {
        context.artifact_state
    };
    Ok(ReviewSubmissionResult {
        review: ReviewRecord {
            schema_version: "human-review-record-v1",
            review_id,
            review_task_id: task.review_task_id,
            review_work_packet_id: task.review_work_packet_id,
            request_id: task.request_id,
            scope_id: task.scope_id,
            artifact_id: task.artifact_id,
            reviewer_actor_id: context.reviewer.reviewer_actor_id,
            source_lineage: context.source_lineage,
            review_types: submission.review_types,
            outcome: submission.outcome,
            recommended_next_state: submission.recommended_next_state,
            created_at: context.now,
            state: ReviewRecordState::Accepted,
        },
        findings,
        review_task_state: ReviewTaskState::Submitted,
        system_completed_review_task_state: ReviewTaskState::Completed,
        artifact_state,
        public_label: derive_public_label(ArtifactPublicationInput {
            state: artifact_state,
            visibility: task.visibility,
            previously_public: promotes,
        }),
    })
}

pub fn validate_review_claim(
    task: &ReviewTaskRecord,
    context: &ReviewClaimContext,
) -> Result<(), ReviewPolicyError> {
    if task.state != ReviewTaskState::Open {
        return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
    }
    if !context.lease_active {
        return Err(ReviewPolicyError::ReviewLeaseNotActive);
    }
    validate_reviewer(task, &context.reviewer, &context.source_lineage)?;
    validate_source_context(context)?;
    Ok(())
}

fn validate_source_context(context: &ReviewClaimContext) -> Result<(), ReviewPolicyError> {
    if !matches!(
        context.artifact_state,
        ArtifactState::MachineValidated | ArtifactState::ReviewRequested
    ) || !source_context_can_promote(context.request_state, context.proposal_state)
    {
        return Err(ReviewPolicyError::ReviewSourceStateNotEligible);
    }
    Ok(())
}

fn source_context_can_promote(
    request_state: RequestState,
    proposal_state: ProposedTaskGraphState,
) -> bool {
    request_state == RequestState::MachineValidated
        && !matches!(
            proposal_state,
            ProposedTaskGraphState::Quarantined | ProposedTaskGraphState::Superseded
        )
}

pub fn derive_public_label(input: ArtifactPublicationInput) -> Option<PublicArtifactLabel> {
    if input.visibility == ArtifactVisibility::Private {
        return None;
    }
    match input.state {
        ArtifactState::MachineValidated | ArtifactState::ReviewRequested => {
            Some(PublicArtifactLabel::MachineValidated)
        }
        ArtifactState::PeerReviewed => Some(PublicArtifactLabel::PeerReviewed),
        ArtifactState::Deprecated if input.previously_public => {
            Some(PublicArtifactLabel::Deprecated)
        }
        ArtifactState::DraftGenerated
        | ArtifactState::ValidationFailed
        | ArtifactState::Quarantined
        | ArtifactState::Deprecated => None,
    }
}

fn validate_reviewer(
    task: &ReviewTaskRecord,
    reviewer: &ReviewerProfile,
    lineage: &SourceActorLineage,
) -> Result<(), ReviewPolicyError> {
    if lineage_identity_metadata_missing(lineage) {
        return Err(ReviewPolicyError::ReviewConflict);
    }
    let direct_conflict = reviewer.reviewer_actor_id == lineage.planner_actor_id
        || reviewer.reviewer_actor_id == lineage.verifier_actor_id
        || reviewer.reviewer_actor_id == lineage.generator_actor_id
        || lineage
            .accepted_output_actors
            .iter()
            .any(|actor| actor.actor_id == reviewer.reviewer_actor_id);
    let operator_conflict = reviewer.operator_account_id == lineage.planner_operator_account_id
        || reviewer.operator_account_id == lineage.verifier_operator_account_id
        || reviewer.operator_account_id == lineage.generator_operator_account_id
        || lineage
            .accepted_output_actors
            .iter()
            .any(|actor| actor.operator_account_id == reviewer.operator_account_id);
    let group_conflict = reviewer.conflict_group_id == lineage.planner_conflict_group_id
        || reviewer.conflict_group_id == lineage.verifier_conflict_group_id
        || reviewer.conflict_group_id == lineage.generator_conflict_group_id
        || lineage
            .accepted_output_actors
            .iter()
            .any(|actor| actor.conflict_group_id == reviewer.conflict_group_id);
    if direct_conflict
        || operator_conflict
        || group_conflict
        || reviewer.actor_type != ActorType::HumanReviewer
        || reviewer.status != ActorStatus::Active
        || reviewer.scope_id != task.scope_id
        || !trust_level_at_least(reviewer.trust_level, task.required_reviewer_trust_level)
        || !reviewer.independence_verified
        || reviewer.operator_account_id.is_empty()
        || reviewer.conflict_group_id.is_empty()
        || !reviewer
            .review_capabilities
            .contains(&ActorCapability::HumanSubjectReview)
        || !reviewer
            .review_capabilities
            .contains(&ActorCapability::HumanPedagogyReview)
        || !reviewer
            .trusted_subjects
            .iter()
            .any(|subject| subject == &task.required_subject)
        || !reviewer
            .trusted_age_ranges
            .iter()
            .any(|age| age == &task.required_age_range)
    {
        return Err(ReviewPolicyError::ReviewConflict);
    }
    Ok(())
}

fn lineage_identity_metadata_missing(lineage: &SourceActorLineage) -> bool {
    lineage.planner_operator_account_id.is_empty()
        || lineage.verifier_operator_account_id.is_empty()
        || lineage.generator_operator_account_id.is_empty()
        || lineage.planner_conflict_group_id.is_empty()
        || lineage.verifier_conflict_group_id.is_empty()
        || lineage.generator_conflict_group_id.is_empty()
        || lineage
            .accepted_output_actors
            .iter()
            .any(|actor| actor.operator_account_id.is_empty() || actor.conflict_group_id.is_empty())
}

fn trust_level_at_least(actual: TrustLevel, required: TrustLevel) -> bool {
    trust_rank(actual) >= trust_rank(required)
}

fn trust_rank(level: TrustLevel) -> u8 {
    match level {
        TrustLevel::Untrusted => 0,
        TrustLevel::RunnerCandidate => 1,
        TrustLevel::ModerationCandidate => 2,
        TrustLevel::PlannerCandidate => 3,
        TrustLevel::VerifierCandidate => 4,
        TrustLevel::ReviewerCandidate => 5,
        TrustLevel::ReviewerApproved => 6,
        TrustLevel::Curator => 7,
        TrustLevel::Admin => 8,
        TrustLevel::System => 9,
    }
}

fn validate_submission(
    task: &ReviewTaskRecord,
    reviewer: &ReviewerProfile,
    submission: &ReviewSubmission,
) -> Result<(), ReviewPolicyError> {
    if !exact_review_types(&submission.review_types) || task.state != ReviewTaskState::Claimed {
        return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
    }
    if let Some(reviewer_id) = &submission.reviewer_id
        && reviewer_id != &reviewer.reviewer_actor_id
    {
        return Err(ReviewPolicyError::ReviewSubmissionLineageMismatch);
    }
    if !submission.submitted_authority_fields.is_empty() {
        return Err(ReviewPolicyError::ReviewSubmittedAuthorityField);
    }
    if let Some(next_state) = &submission.recommended_next_state
        && next_state == "classroom_ready"
    {
        return Err(ReviewPolicyError::ReviewSubmittedAuthorityField);
    }
    if let Some(next_state) = &submission.recommended_next_state
        && !safe_text(next_state)
    {
        return Err(ReviewPolicyError::UnsafeSubmittedReviewText);
    }
    Ok(())
}

fn normalize_findings(
    review_task_id: &ReviewTaskId,
    review_id: &ReviewId,
    created_by_actor_id: &ActorId,
    created_at: u64,
    inputs: &[FindingInput],
) -> Result<Vec<FindingRecord>, ReviewPolicyError> {
    let mut records = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        validate_finding_input(input)?;
        let suffix = index + 1;
        records.push(FindingRecord {
            finding_id: FindingId::try_from(format!(
                "finding_{}_{suffix}",
                review_task_suffix(review_task_id)
            ))
            .map_err(|_| ReviewPolicyError::InvalidFinding)?,
            parent_type: FindingParentType::Review,
            parent_id: review_id.clone(),
            created_by_actor_id: created_by_actor_id.clone(),
            created_at,
            severity: input.severity,
            finding_type: input.finding_type,
            safe_location: input.safe_location.clone(),
            safe_message: input.safe_message.clone(),
            blocking: finding_is_blocking(input.finding_type, input.severity),
            state: FindingState::Open,
        });
    }
    Ok(records)
}

fn review_id_for_task(review_task_id: &ReviewTaskId) -> Result<ReviewId, ReviewPolicyError> {
    ReviewId::try_from(format!("review_{}", review_task_suffix(review_task_id)))
        .map_err(|_| ReviewPolicyError::ReviewSubmissionLineageMismatch)
}

fn review_task_suffix(review_task_id: &ReviewTaskId) -> &str {
    review_task_id
        .as_str()
        .strip_prefix(ReviewTaskId::PREFIX)
        .unwrap_or(review_task_id.as_str())
}

fn validate_finding_input(input: &FindingInput) -> Result<(), ReviewPolicyError> {
    if input.submitted_blocking.is_some() || !input.submitted_authority_fields.is_empty() {
        return Err(ReviewPolicyError::SubmittedFindingAuthorityField);
    }
    if !finding_severity_allowed(input.finding_type, input.severity) {
        return Err(ReviewPolicyError::InvalidFinding);
    }
    if !safe_text(&input.safe_location) || !safe_text(&input.safe_message) {
        return Err(ReviewPolicyError::UnsafeSubmittedReviewText);
    }
    Ok(())
}

fn exact_review_types(values: &[ReviewType]) -> bool {
    values.len() == 2
        && values.contains(&ReviewType::SubjectCorrectness)
        && values.contains(&ReviewType::Pedagogy)
}

fn finding_severity_allowed(finding_type: FindingType, severity: FindingSeverity) -> bool {
    match finding_type {
        FindingType::PiiOrSecretLeak
        | FindingType::UnsafeInstruction
        | FindingType::LicenseIssue
        | FindingType::CheckerMismatch
        | FindingType::MissingAnswerKey => true,
        FindingType::PhysicsError => matches!(
            severity,
            FindingSeverity::Critical | FindingSeverity::Major | FindingSeverity::Minor
        ),
        FindingType::PedagogyIssue => matches!(
            severity,
            FindingSeverity::Major | FindingSeverity::Minor | FindingSeverity::Note
        ),
        FindingType::FormattingIssue | FindingType::AccessibilityNote | FindingType::OtherSafe => {
            matches!(severity, FindingSeverity::Minor | FindingSeverity::Note)
        }
    }
}

fn finding_is_blocking(finding_type: FindingType, severity: FindingSeverity) -> bool {
    match finding_type {
        FindingType::PiiOrSecretLeak
        | FindingType::UnsafeInstruction
        | FindingType::LicenseIssue
        | FindingType::CheckerMismatch
        | FindingType::MissingAnswerKey => true,
        FindingType::PhysicsError => {
            matches!(severity, FindingSeverity::Critical | FindingSeverity::Major)
        }
        FindingType::PedagogyIssue => severity == FindingSeverity::Major,
        FindingType::FormattingIssue | FindingType::AccessibilityNote | FindingType::OtherSafe => {
            false
        }
    }
}

fn safe_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !value.is_empty()
        && value.chars().count() <= 240
        && !lower.contains("http://")
        && !lower.contains("https://")
        && !lower.contains("/home/")
        && !lower.contains("/users/")
        && !lower.contains("/tmp/")
        && !lower.contains("/etc/")
        && !lower.contains("~/")
        && !lower.contains("\\users\\")
        && !lower.contains("c:\\")
        && !lower.contains(".codex")
        && !lower.contains(".ssh")
        && !lower.contains("auth.json")
        && !lower.contains("id_rsa")
        && !lower.contains("library/application support")
        && !lower.contains("sk-")
        && !lower.contains("api_key")
        && !lower.contains("cookie")
        && !lower.contains("credential")
        && !lower.contains("oauth")
        && !lower.contains("token")
        && !lower.contains("password")
        && !lower.contains("prompt")
        && !lower.contains("provider")
        && !lower.contains("secret")
        && !contains_student_pii_marker(value, &lower)
        && !lower.contains("```")
        && !lower.contains("<script")
        && value
            .chars()
            .all(|character| character.is_ascii() && !character.is_control())
}

fn contains_student_pii_marker(value: &str, lower: &str) -> bool {
    lower.contains('@')
        || lower.contains("student")
        || lower.contains("roster")
        || lower.contains("scored")
        || lower.contains("score:")
        || lower.contains("grade")
        || lower.contains("attendance")
        || lower.contains("parent")
        || lower.contains("guardian")
        || lower.contains("alice")
        || lower.contains("bob")
        || contains_percent_grade_marker(value, lower)
        || contains_grade_fraction(value)
        || contains_titlecase_name_pair(value)
        || has_phone_like_number(value, lower)
}

fn contains_percent_grade_marker(value: &str, lower: &str) -> bool {
    let has_grade_context = lower.contains("score")
        || lower.contains("scored")
        || lower.contains("grade")
        || lower.contains("attendance");
    has_grade_context && contains_digit_percent_pair(value)
}

fn contains_digit_percent_pair(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    for (index, character) in chars.iter().enumerate() {
        if *character == '%' {
            let previous_digit = chars[..index]
                .iter()
                .rev()
                .find(|candidate| !candidate.is_ascii_whitespace())
                .is_some_and(|candidate| candidate.is_ascii_digit());
            let next_digit = chars[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_ascii_whitespace())
                .is_some_and(|candidate| candidate.is_ascii_digit());
            if previous_digit || next_digit {
                return true;
            }
        }
    }
    false
}

fn has_phone_like_number(value: &str, lower: &str) -> bool {
    contains_international_phone_candidate(value)
        || contains_unformatted_us_phone_candidate(value, lower)
        || contains_formatted_phone_candidate(value)
        || contains_contextual_phone_candidate(value, lower)
}

fn contains_international_phone_candidate(value: &str) -> bool {
    value.match_indices('+').any(|(plus_index, _)| {
        if previous_char(value, plus_index).is_some_and(|previous| previous.is_ascii_digit()) {
            return false;
        }
        let candidate = &value[plus_index + 1..];
        let end = phone_candidate_end(candidate);
        end > 0 && phone_groups_are_international(&digit_group_lengths(&candidate[..end]))
    })
}

fn contains_unformatted_us_phone_candidate(value: &str, lower: &str) -> bool {
    for (start, character) in value.char_indices() {
        if !character.is_ascii_digit() {
            continue;
        }
        if previous_char(value, start).is_some_and(|previous| previous.is_ascii_digit()) {
            continue;
        }
        let candidate = &value[start..];
        let end = phone_candidate_end(candidate);
        if end == 0 {
            continue;
        }
        let base = &candidate[..end];
        let groups = digit_group_lengths(base);
        if matches!(groups.as_slice(), [11]) && base.starts_with('1') {
            return true;
        }
        if matches!(groups.as_slice(), [10]) && identifier_context_before_candidate(lower, start) {
            continue;
        }
        let context_before = phone_context_before_candidate(lower, start);
        let context_after = phone_context_after_candidate(lower, start + end);
        let extension_after = phone_extension_after_candidate(lower, start + end);
        if matches!(groups.as_slice(), [10])
            && (context_before || context_after || extension_after || !base.starts_with('0'))
        {
            return true;
        }
    }
    false
}

fn contains_formatted_phone_candidate(value: &str) -> bool {
    for (start, character) in value.char_indices() {
        if !character.is_ascii_digit() && character != '(' {
            continue;
        }
        if previous_char(value, start).is_some_and(|previous| previous.is_ascii_digit()) {
            continue;
        }
        let candidate = &value[start..];
        let end = phone_candidate_end(candidate);
        if end == 0 {
            continue;
        }
        let base = &candidate[..end];
        let groups = digit_group_lengths(base);
        if phone_candidate_has_separator(base)
            && matches!(groups.as_slice(), [3, 3, 4] | [1, 3, 3, 4])
        {
            return true;
        }
    }
    false
}

fn contains_contextual_phone_candidate(value: &str, lower: &str) -> bool {
    for (start, character) in value.char_indices() {
        if !character.is_ascii_digit() && character != '(' {
            continue;
        }
        if previous_char(value, start).is_some_and(|previous| previous.is_ascii_digit()) {
            continue;
        }
        let candidate = &value[start..];
        let end = phone_candidate_end(candidate);
        if end == 0 {
            continue;
        }
        let context_before = phone_context_before_candidate(lower, start);
        let context_after = phone_context_after_candidate(lower, start + end);
        if !context_before && !context_after {
            continue;
        }
        let groups = digit_group_lengths(&candidate[..end]);
        if matches!(groups.as_slice(), [7] | [3, 4] | [10] | [11])
            || phone_groups_are_international(&groups)
        {
            return true;
        }
    }
    false
}

fn phone_groups_are_international(groups: &[usize]) -> bool {
    let digit_count = groups.iter().copied().sum::<usize>();
    (8..=15).contains(&digit_count)
        && (groups.len() == 1
            || (groups.len() == 2 && (1..=3).contains(&groups[0]))
            || groups.len() >= 3
            || matches!(groups, [4, 4, 4]))
}

fn phone_context_before_candidate(lowercase: &str, candidate_start: usize) -> bool {
    let prefix = lowercase[..candidate_start].trim_end_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.' | '(')
    });
    let mut words = prefix
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    while words
        .last()
        .is_some_and(|word| matches!(*word, "is" | "at"))
    {
        words.pop();
    }
    contact_context_suffixes()
        .iter()
        .any(|suffix| words.ends_with(suffix))
}

fn phone_context_after_candidate(lowercase: &str, candidate_end: usize) -> bool {
    let suffix = lowercase[candidate_end..].trim_start_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.' | '/' | ')')
    });
    let words = suffix
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(3)
        .collect::<Vec<_>>();
    let contact_prefixes: &[&[&str]] = &[
        &["phone"],
        &["phone", "number"],
        &["mobile"],
        &["mobile", "number"],
        &["cell"],
        &["cell", "number"],
        &["cell", "phone", "number"],
        &["telephone"],
        &["telephone", "number"],
        &["tel"],
    ];
    contact_prefixes
        .iter()
        .any(|prefix| words.starts_with(prefix))
}

fn phone_extension_after_candidate(lowercase: &str, candidate_end: usize) -> bool {
    let suffix = lowercase[candidate_end..].trim_start_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.')
    });
    suffix.starts_with('x') || suffix.starts_with("ext") || suffix.starts_with("extension")
}

fn identifier_context_before_candidate(lowercase: &str, candidate_start: usize) -> bool {
    let prefix = lowercase[..candidate_start].trim_end_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.')
    });
    let words = prefix
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    words.ends_with(&["isbn"]) || words.ends_with(&["isbn", "10"])
}

fn contact_context_suffixes() -> &'static [&'static [&'static str]] {
    &[
        &["phone"],
        &["phone", "number"],
        &["mobile"],
        &["mobile", "number"],
        &["cell"],
        &["cell", "number"],
        &["cell", "phone", "number"],
        &["telephone"],
        &["telephone", "number"],
        &["text"],
        &["text", "me"],
        &["text", "me", "at"],
        &["sms"],
        &["sms", "at"],
        &["call"],
        &["call", "me"],
        &["call", "me", "at"],
        &["contact"],
        &["contact", "me"],
        &["contact", "me", "at"],
        &["contact", "phone"],
        &["tel"],
    ]
}

fn phone_candidate_has_separator(candidate: &str) -> bool {
    candidate
        .chars()
        .any(|character| matches!(character, ' ' | '-' | '.' | '/' | '(' | ')'))
}

fn phone_candidate_end(candidate: &str) -> usize {
    candidate
        .char_indices()
        .find_map(|(index, character)| (!is_phone_base_character(character)).then_some(index))
        .unwrap_or(candidate.len())
}

fn is_phone_base_character(character: char) -> bool {
    character.is_ascii_digit() || matches!(character, ' ' | '-' | '.' | '/' | '(' | ')')
}

fn digit_group_lengths(base: &str) -> Vec<usize> {
    base.split(|character: char| !character.is_ascii_digit())
        .filter(|group| !group.is_empty())
        .map(str::len)
        .collect()
}

fn previous_char(value: &str, index: usize) -> Option<char> {
    value[..index].chars().next_back()
}

fn contains_titlecase_name_pair(value: &str) -> bool {
    let mut previous_was_name = false;
    for word in value.split(|character: char| !character.is_ascii_alphabetic()) {
        if word.is_empty() {
            continue;
        }
        let current_is_name = titlecase_word_shape(word);
        if previous_was_name && current_is_name {
            return true;
        }
        previous_was_name = current_is_name;
    }
    false
}

fn contains_grade_fraction(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    chars
        .windows(3)
        .any(|window| window[0].is_ascii_digit() && window[1] == '/' && window[2].is_ascii_digit())
}

fn titlecase_word_shape(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    word.len() >= 2 && first.is_ascii_uppercase() && chars.all(|ch| ch.is_ascii_lowercase())
}
