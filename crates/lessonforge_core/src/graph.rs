use crate::ids::{
    ActorId, PlanVerificationTaskId, PlanningTaskId, PromotionDecisionId, ProposedTaskGraphId,
    RequestId, WorkPacketId,
};
use crate::state::{PlanVerificationTaskState, ProposedTaskGraphState, WorkPacketState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const CENTRAL_VALIDATOR_VERSION: &str = "lessonforge_graph_policy_v1";
const MVP_SCHEMA_VERSION: &str = "1.0";
const PLAN_CROSS_CHECK: &str = "plan_schema_policy_cross_check";
const PEER_REVIEWED: &str = "peer_reviewed";
const LOW_RISK: &str = "low";

const VALIDATION_CHECKS: &[&str] = &[
    "manifest_schema",
    "required_files",
    "license_metadata",
    "ai_assistance_disclosure",
    "obvious_pii_heuristic",
    "obvious_inappropriate_content_heuristic",
    "python_checker_runs",
    "no_external_network_static",
];

const MVP_ARTIFACTS: &[&str] = &["worksheet", "answer_key", "python_checker", "teacher_notes"];
const GENERATION_OUTPUTS: &[&str] = &[
    "worksheet.md",
    "answer_key.md",
    "checker.py",
    "teacher_notes.md",
    "manifest.json",
];
const VALIDATION_OUTPUTS: &[&str] = &["validation_report.json"];
const REVIEW_OUTPUTS: &[&str] = &["review.json"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphValidationContext {
    pub request_id: RequestId,
    pub planning_task_id: PlanningTaskId,
    pub planner_actor_id: ActorId,
    pub scope_id: String,
    pub subject: String,
    pub topic: String,
    pub age_range: String,
    pub language: String,
    pub lesson_duration_minutes: u16,
    pub plan_verification_task_id: PlanVerificationTaskId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphValidationOutcome {
    pub decision: ValidationDecision,
    pub proposal: ProposedTaskGraphRecord,
    pub plan_verification_task: Option<PlanVerificationTaskRecord>,
    pub errors: Vec<GraphPolicyError>,
    pub keeps_planning_fanout_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationDecision {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedTaskGraphRecord {
    proposal_id: ProposedTaskGraphId,
    request_id: RequestId,
    planning_task_id: PlanningTaskId,
    planner_actor_id: ActorId,
    scope_id: String,
    schema_version: String,
    central_validator_version: String,
    state: ProposedTaskGraphState,
    central_risk_level: String,
    tasks: Vec<NormalizedTaskRecord>,
    plan_verification_task_id: Option<PlanVerificationTaskId>,
    mvp_policy_fingerprint: Option<String>,
}

impl ProposedTaskGraphRecord {
    pub fn proposal_id(&self) -> &ProposedTaskGraphId {
        &self.proposal_id
    }

    pub fn state(&self) -> ProposedTaskGraphState {
        self.state
    }

    fn bind_rejected_lineage_to_context(&mut self, context: &GraphValidationContext) {
        self.request_id = context.request_id.clone();
        self.planning_task_id = context.planning_task_id.clone();
        self.planner_actor_id = context.planner_actor_id.clone();
        self.plan_verification_task_id = None;
        self.mvp_policy_fingerprint = None;
    }

    pub fn central_risk_level(&self) -> &str {
        &self.central_risk_level
    }

    pub fn apply_plan_verification(
        &self,
        evidence: PlanVerificationEvidence,
    ) -> Result<Self, GraphPolicyError> {
        if self.state != ProposedTaskGraphState::VerificationRequired {
            return Err(GraphPolicyError::new(
                "proposal_not_pending_plan_verification",
                "/proposal/state",
            ));
        }
        if self.plan_verification_task_id.as_ref() != Some(&evidence.plan_verification_task_id) {
            return Err(GraphPolicyError::new(
                "verification_task_lineage_mismatch",
                "/verification/plan_verification_task_id",
            ));
        }
        if evidence.proposal_id != self.proposal_id
            || evidence.request_id != self.request_id
            || evidence.planning_task_id != self.planning_task_id
            || evidence.verification_type != PLAN_CROSS_CHECK
        {
            return Err(GraphPolicyError::new(
                "verification_lineage_mismatch",
                "/verification/lineage",
            ));
        }
        if !evidence.verifier_lease_active_and_consumed {
            return Err(GraphPolicyError::new(
                "verification_lease_not_consumed",
                "/verification/lease",
            ));
        }
        if evidence.verifier_actor_id == self.planner_actor_id {
            return Err(GraphPolicyError::new(
                "planner_cannot_verify_own_proposal",
                "/verification/verifier_actor_id",
            ));
        }
        if evidence.verifier_scope_id != self.scope_id {
            return Err(GraphPolicyError::new(
                "verification_scope_mismatch",
                "/verification/scope_id",
            ));
        }

        let mut verified = self.clone();
        verified.state = match evidence.outcome {
            PlanVerificationOutcome::NoBlockingFindings
                if !evidence.blocking_findings && !evidence.critical_or_major_findings =>
            {
                ProposedTaskGraphState::VerifiedForMvpPromotion
            }
            _ => ProposedTaskGraphState::VerificationBlocked,
        };
        Ok(verified)
    }

    pub fn require_plan_verification(&self) -> Result<Self, GraphPolicyError> {
        if self.state != ProposedTaskGraphState::SchemaPolicyValidated {
            return Err(GraphPolicyError::new(
                "proposal_not_schema_policy_validated",
                "/proposal/state",
            ));
        }
        if self.plan_verification_task_id.is_none() {
            return Err(GraphPolicyError::new(
                "verification_task_lineage_missing",
                "/proposal/plan_verification_task_id",
            ));
        }
        let mut proposal = self.clone();
        proposal.state = ProposedTaskGraphState::VerificationRequired;
        Ok(proposal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedTaskRecord {
    pub local_id: String,
    pub phase: String,
    pub task_type: String,
    pub required_capabilities: Vec<String>,
    pub outputs: Vec<String>,
    pub depends_on: Vec<String>,
    pub validation_required: Vec<String>,
    pub human_review_required_for: Vec<String>,
    pub execution_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanVerificationTaskRecord {
    pub plan_verification_task_id: PlanVerificationTaskId,
    pub proposal_id: ProposedTaskGraphId,
    pub request_id: RequestId,
    pub planning_task_id: PlanningTaskId,
    pub state: PlanVerificationTaskState,
    pub verification_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanVerificationEvidence {
    pub plan_verification_task_id: PlanVerificationTaskId,
    pub proposal_id: ProposedTaskGraphId,
    pub request_id: RequestId,
    pub planning_task_id: PlanningTaskId,
    pub verification_type: String,
    pub verifier_lease_active_and_consumed: bool,
    pub verifier_actor_id: ActorId,
    pub verifier_scope_id: String,
    pub outcome: PlanVerificationOutcome,
    pub blocking_findings: bool,
    pub critical_or_major_findings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanVerificationOutcome {
    NoBlockingFindings,
    BlockingFindings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionContext {
    pub request_id: RequestId,
    pub scope_id: String,
    pub planning_task_completed_for_proposal: bool,
    pub request_available_for_promotion: bool,
    pub active_competing_planning_leases: u16,
    pub verification: Option<PlanVerificationOutcome>,
    pub open_blocking_findings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionIds {
    pub promotion_decision_id: PromotionDecisionId,
    pub work_packet_ids: Vec<WorkPacketId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromotionLedger {
    decisions_by_command: BTreeMap<String, PromotionReplayRecord>,
    decisions_by_proposal: BTreeMap<ProposedTaskGraphId, PromotionDecisionRecord>,
    work_packets_by_source: BTreeSet<(ProposedTaskGraphId, String)>,
}

impl PromotionLedger {
    pub fn work_packet_count(&self) -> usize {
        self.work_packets_by_source.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromotionReplayRecord {
    request_fingerprint: String,
    decision: PromotionDecisionRecord,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphValidationLedger {
    verification_tasks: BTreeMap<(ProposedTaskGraphId, &'static str), PlanVerificationTaskRecord>,
    proposal_fingerprints: BTreeMap<ProposedTaskGraphId, String>,
}

impl GraphValidationLedger {
    pub fn plan_verification_task_count(&self) -> usize {
        self.verification_tasks.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionDecisionRecord {
    pub promotion_decision_id: PromotionDecisionId,
    pub state: PromotionDecisionState,
    pub proposal: ProposedTaskGraphRecord,
    pub work_packets: Vec<WorkPacketRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionDecisionState {
    Accepted,
    AlreadyPromoted,
}

impl PromotionDecisionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::AlreadyPromoted => "already_promoted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkPacketRecord {
    pub work_packet_id: WorkPacketId,
    pub proposal_id: ProposedTaskGraphId,
    pub source_local_task_id: String,
    pub state: WorkPacketState,
    pub depends_on_work_packet_ids: Vec<WorkPacketId>,
    pub snapshot: NormalizedTaskRecord,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GraphPolicyError {
    pub code: &'static str,
    pub field_path: String,
}

impl GraphPolicyError {
    fn new(code: &'static str, field_path: impl Into<String>) -> Self {
        Self {
            code,
            field_path: field_path.into(),
        }
    }
}

impl fmt::Debug for GraphPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GraphPolicyError")
            .field("code", &self.code)
            .field("field_path", &self.field_path)
            .finish()
    }
}

impl fmt::Display for GraphPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.code, self.field_path)
    }
}

impl std::error::Error for GraphPolicyError {}

pub fn validate_proposed_task_graph(
    input: Value,
    context: GraphValidationContext,
) -> Result<GraphValidationOutcome, GraphPolicyError> {
    validate_proposed_task_graph_inner(input, context, None)
}

pub fn validate_proposed_task_graph_with_ledger(
    input: Value,
    context: GraphValidationContext,
    ledger: &mut GraphValidationLedger,
) -> Result<GraphValidationOutcome, GraphPolicyError> {
    validate_proposed_task_graph_inner(input, context, Some(ledger))
}

fn validate_proposed_task_graph_inner(
    input: Value,
    context: GraphValidationContext,
    ledger: Option<&mut GraphValidationLedger>,
) -> Result<GraphValidationOutcome, GraphPolicyError> {
    let fallback_proposal_id = ProposedTaskGraphId::try_from("plan_rejected")?;
    let submitted_proposal_id = input
        .get("proposal_id")
        .and_then(Value::as_str)
        .and_then(|value| ProposedTaskGraphId::try_from(value).ok())
        .unwrap_or(fallback_proposal_id);

    if let Some(error) = first_shape_error(&input) {
        return Ok(rejected_outcome(
            submitted_proposal_id,
            &context,
            ProposedTaskGraphState::SchemaRejected,
            error,
        ));
    }
    if let Some(error) = first_forbidden_field_error(&input) {
        return Ok(rejected_outcome(
            submitted_proposal_id,
            &context,
            ProposedTaskGraphState::PolicyRejected,
            error,
        ));
    }

    let submitted: ProposedTaskGraphInput = match serde_json::from_value(input) {
        Ok(submitted) => submitted,
        Err(_) => {
            return Ok(rejected_outcome(
                submitted_proposal_id,
                &context,
                ProposedTaskGraphState::SchemaRejected,
                GraphPolicyError::new("proposed_graph_schema_deserialize_failed", "/"),
            ));
        }
    };
    let mut proposal = match proposal_record(&submitted, &context) {
        Ok(proposal) => proposal,
        Err(error) => {
            return Ok(rejected_outcome(
                submitted_proposal_id,
                &context,
                ProposedTaskGraphState::SchemaRejected,
                error,
            ));
        }
    };

    let mut errors = Vec::new();
    collect_policy_errors(&submitted, &context, &mut errors);
    if errors.is_empty() {
        proposal.state = ProposedTaskGraphState::SchemaPolicyValidated;
        proposal.plan_verification_task_id = Some(context.plan_verification_task_id.clone());
        proposal.mvp_policy_fingerprint = Some(mvp_policy_fingerprint(&proposal.tasks));
        let plan_verification_task =
            plan_verification_task_for(&proposal, &submitted, &context, ledger)?;
        return Ok(GraphValidationOutcome {
            decision: ValidationDecision::Accepted,
            proposal: proposal.clone(),
            plan_verification_task: Some(plan_verification_task),
            errors,
            keeps_planning_fanout_open: false,
        });
    }

    proposal.state = ProposedTaskGraphState::PolicyRejected;
    proposal.bind_rejected_lineage_to_context(&context);
    proposal.central_risk_level = central_risk_level(&submitted);
    proposal.tasks.clear();
    Ok(GraphValidationOutcome {
        decision: ValidationDecision::Rejected,
        proposal,
        plan_verification_task: None,
        errors,
        keeps_planning_fanout_open: true,
    })
}

pub fn promote_verified_proposal(
    proposal: &ProposedTaskGraphRecord,
    context: PromotionContext,
    ids: PromotionIds,
    command_id: &str,
    _request_fingerprint: &str,
    ledger: &mut PromotionLedger,
) -> Result<PromotionDecisionRecord, GraphPolicyError> {
    let request_fingerprint = promotion_fingerprint(proposal, &context, &ids);
    validate_promotion_preconditions(proposal, &context, &ids)?;

    if let Some(existing) = ledger.decisions_by_command.get(command_id) {
        if existing.request_fingerprint == request_fingerprint {
            return Ok(existing.decision.clone());
        }
        return Err(GraphPolicyError::new(
            "idempotency_key_reused_with_changed_promotion",
            "/promotion/command_id",
        ));
    }

    if let Some(existing) = ledger.decisions_by_proposal.get(&proposal.proposal_id) {
        let mut no_op = existing.clone();
        no_op.state = PromotionDecisionState::AlreadyPromoted;
        return Ok(no_op);
    }

    let work_packets = materialize_work_packets(proposal, &ids)?;
    let new_source_keys: BTreeSet<(ProposedTaskGraphId, String)> = work_packets
        .iter()
        .map(|packet| {
            (
                packet.proposal_id.clone(),
                packet.source_local_task_id.clone(),
            )
        })
        .collect();
    if new_source_keys.len() != work_packets.len()
        || new_source_keys
            .iter()
            .any(|key| ledger.work_packets_by_source.contains(key))
    {
        return Err(GraphPolicyError::new(
            "duplicate_work_packet_source",
            "/promotion/work_packets",
        ));
    }

    for key in &new_source_keys {
        ledger.work_packets_by_source.insert(key.clone());
    }
    let decision = PromotionDecisionRecord {
        promotion_decision_id: ids.promotion_decision_id,
        state: PromotionDecisionState::Accepted,
        proposal: promoted_proposal(proposal)?,
        work_packets,
    };
    ledger
        .decisions_by_proposal
        .insert(proposal.proposal_id.clone(), decision.clone());
    ledger.decisions_by_command.insert(
        command_id.to_owned(),
        PromotionReplayRecord {
            request_fingerprint,
            decision: decision.clone(),
        },
    );
    Ok(decision)
}

fn plan_verification_task_for(
    proposal: &ProposedTaskGraphRecord,
    submitted: &ProposedTaskGraphInput,
    context: &GraphValidationContext,
    ledger: Option<&mut GraphValidationLedger>,
) -> Result<PlanVerificationTaskRecord, GraphPolicyError> {
    let key = (proposal.proposal_id.clone(), PLAN_CROSS_CHECK);
    let submitted_fingerprint = proposal_submission_fingerprint(submitted)?;
    if let Some(ledger) = ledger {
        if let Some(existing) = ledger.proposal_fingerprints.get(&proposal.proposal_id)
            && existing != &submitted_fingerprint
        {
            return Err(GraphPolicyError::new(
                "proposal_replay_changed",
                "/proposal_id",
            ));
        }
        if let Some(existing) = ledger.verification_tasks.get(&key) {
            return Ok(existing.clone());
        }
        let created = PlanVerificationTaskRecord {
            plan_verification_task_id: context.plan_verification_task_id.clone(),
            proposal_id: proposal.proposal_id.clone(),
            request_id: context.request_id.clone(),
            planning_task_id: context.planning_task_id.clone(),
            state: PlanVerificationTaskState::Open,
            verification_type: PLAN_CROSS_CHECK,
        };
        ledger
            .proposal_fingerprints
            .insert(proposal.proposal_id.clone(), submitted_fingerprint);
        ledger.verification_tasks.insert(key, created.clone());
        Ok(created)
    } else {
        Ok(PlanVerificationTaskRecord {
            plan_verification_task_id: context.plan_verification_task_id.clone(),
            proposal_id: proposal.proposal_id.clone(),
            request_id: context.request_id.clone(),
            planning_task_id: context.planning_task_id.clone(),
            state: PlanVerificationTaskState::Open,
            verification_type: PLAN_CROSS_CHECK,
        })
    }
}

fn promoted_proposal(
    proposal: &ProposedTaskGraphRecord,
) -> Result<ProposedTaskGraphRecord, GraphPolicyError> {
    let mut promoted = proposal.clone();
    promoted.state = ProposedTaskGraphState::Promoted;
    Ok(promoted)
}

fn proposal_submission_fingerprint(
    submitted: &ProposedTaskGraphInput,
) -> Result<String, GraphPolicyError> {
    let encoded = serde_json::to_vec(submitted)
        .map_err(|_| GraphPolicyError::new("proposal_fingerprint_failed", "/proposal_id"))?;
    Ok(sha256_hex(&encoded))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn promotion_fingerprint(
    proposal: &ProposedTaskGraphRecord,
    context: &PromotionContext,
    ids: &PromotionIds,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{:?}|{:?}",
        proposal.proposal_id,
        proposal.state.as_str(),
        proposal
            .mvp_policy_fingerprint
            .as_deref()
            .unwrap_or("missing"),
        context.request_id,
        context.scope_id,
        context.planning_task_completed_for_proposal,
        context.request_available_for_promotion,
        context.active_competing_planning_leases,
        context.verification,
        context.open_blocking_findings,
        ids.promotion_decision_id,
        ids.work_packet_ids
    )
}

fn mvp_policy_fingerprint(tasks: &[NormalizedTaskRecord]) -> String {
    tasks
        .iter()
        .map(|task| {
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}",
                task.local_id,
                task.phase,
                task.task_type,
                task.required_capabilities.join(","),
                task.outputs.join(","),
                task.depends_on.join(","),
                task.validation_required.join(","),
                task.execution_policy.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn first_shape_error(input: &Value) -> Option<GraphPolicyError> {
    let object = input.as_object()?;
    let allowed = BTreeSet::from([
        "proposal_id",
        "request_id",
        "planning_task_id",
        "planner_runner_id",
        "schema_version",
        "status",
        "source_request_summary",
        "assumptions",
        "missing_information",
        "proposed_artifacts",
        "proposed_tasks",
        "validation_plan",
        "human_review_required_for",
    ]);
    if object.keys().any(|key| !allowed.contains(key.as_str())) {
        return Some(GraphPolicyError::new("unknown_top_level_field", "/"));
    }
    for required in allowed {
        if !object.contains_key(required) {
            return Some(GraphPolicyError::new("missing_required_field", "/"));
        }
    }
    None
}

fn first_forbidden_field_error(value: &Value) -> Option<GraphPolicyError> {
    first_forbidden_field_error_at(value, "")
}

fn first_forbidden_field_error_at(value: &Value, path: &str) -> Option<GraphPolicyError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                let key_path = json_pointer_child(path, key);
                if is_forbidden_key(key) {
                    return Some(GraphPolicyError::new("forbidden_runner_field", key_path));
                }
                if let Some(error) = first_forbidden_field_error_at(nested, &key_path) {
                    return Some(error);
                }
            }
            None
        }
        Value::Array(items) => items.iter().enumerate().find_map(|(index, nested)| {
            first_forbidden_field_error_at(nested, &format!("{path}/{index}"))
        }),
        _ => None,
    }
}

fn json_pointer_child(parent: &str, key: &str) -> String {
    format!("{parent}/{}", escape_json_pointer_segment(key))
}

fn escape_json_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn is_forbidden_key(key: &str) -> bool {
    matches!(
        key,
        "prompt"
            | "arbitrary_prompt"
            | "system_prompt"
            | "developer_prompt"
            | "user_prompt"
            | "hidden_instruction"
            | "model"
            | "provider"
            | "api_key"
            | "token"
            | "credential"
            | "local_path"
            | "auth_path"
            | "cookie"
            | "headers"
            | "tool_call"
            | "function_call"
            | "webhook"
            | "url"
            | "attachment"
            | "student_names"
            | "student_records"
    ) || key.as_bytes()
        == [
            112, 114, 111, 118, 105, 100, 101, 114, 95, 98, 97, 115, 101, 95, 117, 114, 108,
        ]
}

fn rejected_outcome(
    proposal_id: ProposedTaskGraphId,
    context: &GraphValidationContext,
    state: ProposedTaskGraphState,
    error: GraphPolicyError,
) -> GraphValidationOutcome {
    GraphValidationOutcome {
        decision: ValidationDecision::Rejected,
        proposal: ProposedTaskGraphRecord {
            proposal_id,
            request_id: context.request_id.clone(),
            planning_task_id: context.planning_task_id.clone(),
            planner_actor_id: context.planner_actor_id.clone(),
            scope_id: context.scope_id.clone(),
            schema_version: MVP_SCHEMA_VERSION.to_owned(),
            central_validator_version: CENTRAL_VALIDATOR_VERSION.to_owned(),
            state,
            central_risk_level: "unknown".to_owned(),
            tasks: Vec::new(),
            plan_verification_task_id: None,
            mvp_policy_fingerprint: None,
        },
        plan_verification_task: None,
        errors: vec![error],
        keeps_planning_fanout_open: true,
    }
}

fn proposal_record(
    submitted: &ProposedTaskGraphInput,
    context: &GraphValidationContext,
) -> Result<ProposedTaskGraphRecord, GraphPolicyError> {
    Ok(ProposedTaskGraphRecord {
        proposal_id: ProposedTaskGraphId::try_from(submitted.proposal_id.as_str())?,
        request_id: RequestId::try_from(submitted.request_id.as_str())?,
        planning_task_id: PlanningTaskId::try_from(submitted.planning_task_id.as_str())?,
        planner_actor_id: ActorId::try_from(submitted.planner_runner_id.as_str())?,
        scope_id: context.scope_id.clone(),
        schema_version: submitted.schema_version.clone(),
        central_validator_version: CENTRAL_VALIDATOR_VERSION.to_owned(),
        state: ProposedTaskGraphState::Proposed,
        central_risk_level: LOW_RISK.to_owned(),
        tasks: submitted
            .proposed_tasks
            .iter()
            .map(NormalizedTaskRecord::from)
            .collect(),
        plan_verification_task_id: None,
        mvp_policy_fingerprint: None,
    })
}

fn collect_policy_errors(
    submitted: &ProposedTaskGraphInput,
    context: &GraphValidationContext,
    errors: &mut Vec<GraphPolicyError>,
) {
    require(
        submitted.schema_version == MVP_SCHEMA_VERSION,
        "unsupported_proposed_graph_schema_version",
        "/schema_version",
        errors,
    );
    require(
        submitted.status == "proposed",
        "invalid_submitted_proposal_status",
        "/status",
        errors,
    );
    require(
        RequestId::try_from(submitted.request_id.as_str())
            .ok()
            .as_ref()
            == Some(&context.request_id),
        "proposal_request_lineage_mismatch",
        "/request_id",
        errors,
    );
    require(
        PlanningTaskId::try_from(submitted.planning_task_id.as_str())
            .ok()
            .as_ref()
            == Some(&context.planning_task_id),
        "proposal_planning_lineage_mismatch",
        "/planning_task_id",
        errors,
    );
    require(
        ActorId::try_from(submitted.planner_runner_id.as_str())
            .ok()
            .as_ref()
            == Some(&context.planner_actor_id),
        "proposal_planner_actor_mismatch",
        "/planner_runner_id",
        errors,
    );
    validate_summary(&submitted.source_request_summary, context, errors);
    validate_bounded_text_list(&submitted.assumptions, "/assumptions", errors);
    validate_bounded_text_list(
        &submitted.missing_information,
        "/missing_information",
        errors,
    );
    validate_exact_set(
        &submitted.validation_plan,
        VALIDATION_CHECKS,
        "invalid_validation_plan",
        "/validation_plan",
        errors,
    );
    validate_exact_set(
        &submitted.human_review_required_for,
        &[PEER_REVIEWED],
        "missing_human_review_gate",
        "/human_review_required_for",
        errors,
    );
    validate_artifacts(&submitted.proposed_artifacts, errors);
    validate_tasks(&submitted.proposed_tasks, context, errors);
}

fn validate_summary(
    summary: &SourceRequestSummaryInput,
    context: &GraphValidationContext,
    errors: &mut Vec<GraphPolicyError>,
) {
    require(
        summary.subject == context.subject,
        "proposal_request_summary_mismatch",
        "/source_request_summary/subject",
        errors,
    );
    require(
        summary.topic == context.topic,
        "proposal_request_summary_mismatch",
        "/source_request_summary/topic",
        errors,
    );
    require(
        summary.age_range == context.age_range,
        "proposal_request_summary_mismatch",
        "/source_request_summary/age_range",
        errors,
    );
    require(
        summary.duration_minutes == context.lesson_duration_minutes,
        "proposal_request_summary_mismatch",
        "/source_request_summary/duration_minutes",
        errors,
    );
    require(
        summary.language == context.language,
        "proposal_request_summary_mismatch",
        "/source_request_summary/language",
        errors,
    );
}

fn validate_bounded_text_list(
    values: &[String],
    field_path: &'static str,
    errors: &mut Vec<GraphPolicyError>,
) {
    require(
        values.len() <= 12,
        "explanatory_list_too_long",
        field_path,
        errors,
    );
    for value in values {
        require(
            value.chars().count() <= 240,
            "explanatory_item_too_long",
            field_path,
            errors,
        );
    }
}

fn validate_artifacts(artifacts: &[ProposedArtifactInput], errors: &mut Vec<GraphPolicyError>) {
    let required_artifacts: Vec<String> = artifacts
        .iter()
        .filter(|artifact| artifact.priority == "required")
        .map(|artifact| artifact.artifact_type.clone())
        .collect();
    validate_exact_set(
        &required_artifacts,
        MVP_ARTIFACTS,
        "invalid_required_artifacts",
        "/proposed_artifacts",
        errors,
    );
    require(
        artifacts.len() == MVP_ARTIFACTS.len(),
        "invalid_required_artifacts",
        "/proposed_artifacts",
        errors,
    );
}

fn validate_tasks(
    tasks: &[ProposedTaskInput],
    context: &GraphValidationContext,
    errors: &mut Vec<GraphPolicyError>,
) {
    let by_id: BTreeMap<&str, &ProposedTaskInput> = tasks
        .iter()
        .map(|task| (task.local_id.as_str(), task))
        .collect();
    require(
        by_id.len() == tasks.len(),
        "duplicate_task_local_id",
        "/proposed_tasks",
        errors,
    );
    require(
        tasks.len() == 3,
        "invalid_mvp_task_count",
        "/proposed_tasks",
        errors,
    );

    for task in tasks {
        validate_task_basics(task, context, &by_id, errors);
    }
    validate_graph_shape(&by_id, errors);
}

fn central_risk_level(submitted: &ProposedTaskGraphInput) -> String {
    if submitted
        .proposed_tasks
        .iter()
        .any(|task| task.risk_level == "high" || is_high_risk_task_type(&task.task_type))
    {
        "high".to_owned()
    } else if submitted
        .proposed_tasks
        .iter()
        .any(|task| task.risk_level == "medium" || has_medium_risk_shape(task))
    {
        "medium".to_owned()
    } else {
        LOW_RISK.to_owned()
    }
}

fn has_medium_risk_shape(task: &ProposedTaskInput) -> bool {
    task.outputs
        .iter()
        .any(|output| output.ends_with(".py") && output != "checker.py")
        || task
            .execution_policy
            .as_deref()
            .is_some_and(|policy| policy != "code_generation_only" && !policy.is_empty())
}

fn is_high_risk_task_type(task_type: &str) -> bool {
    matches!(
        task_type,
        "arbitrary_prompt"
            | "credential_handling_task"
            | "student_grading_task"
            | "student_placement_task"
            | "student_profile_task"
            | "send_email"
            | "web_fetch"
            | "browser_browse"
            | "install_dependency"
            | "execute_shell"
            | "payment_or_subscription"
            | "collect_student_data"
            | "publish_without_review"
    ) || task_type.as_bytes()
        == [
            112, 114, 111, 118, 105, 100, 101, 114, 95, 97, 99, 99, 111, 117, 110, 116, 95, 115,
            101, 116, 117, 112,
        ]
}

fn validate_task_basics(
    task: &ProposedTaskInput,
    context: &GraphValidationContext,
    by_id: &BTreeMap<&str, &ProposedTaskInput>,
    errors: &mut Vec<GraphPolicyError>,
) {
    require(
        valid_local_id(&task.local_id),
        "invalid_task_local_id",
        "/proposed_tasks/local_id",
        errors,
    );
    require(
        task.subject == context.subject,
        "proposal_task_request_mismatch",
        "/proposed_tasks/subject",
        errors,
    );
    require(
        task.topic == context.topic,
        "proposal_task_request_mismatch",
        "/proposed_tasks/topic",
        errors,
    );
    require(
        task.age_range == context.age_range,
        "proposal_task_request_mismatch",
        "/proposed_tasks/age_range",
        errors,
    );
    require(
        task.language == context.language,
        "proposal_task_request_mismatch",
        "/proposed_tasks/language",
        errors,
    );
    require(
        task.risk_level == LOW_RISK,
        "medium_risk_requires_future_policy",
        "/proposed_tasks/risk_level",
        errors,
    );
    for dependency in &task.depends_on {
        require(
            by_id.contains_key(dependency.as_str()),
            "unknown_task_dependency",
            "/proposed_tasks/depends_on",
            errors,
        );
        require(
            dependency != &task.local_id,
            "self_task_dependency",
            "/proposed_tasks/depends_on",
            errors,
        );
    }
    let unique_deps: BTreeSet<&str> = task.depends_on.iter().map(String::as_str).collect();
    require(
        unique_deps.len() == task.depends_on.len(),
        "duplicate_task_dependency",
        "/proposed_tasks/depends_on",
        errors,
    );

    match task.task_type.as_str() {
        "generate_lesson_pack" => validate_generation_task(task, errors),
        "run_artifact_validation" => validate_validation_task(task, errors),
        "review_subject_and_pedagogy" => validate_review_task(task, errors),
        _ if is_high_risk_task_type(&task.task_type) => require(
            false,
            "high_risk_task_rejected",
            "/proposed_tasks/task_type",
            errors,
        ),
        _ => require(
            false,
            "unknown_task_type",
            "/proposed_tasks/task_type",
            errors,
        ),
    }
}

fn validate_generation_task(task: &ProposedTaskInput, errors: &mut Vec<GraphPolicyError>) {
    require(
        task.phase == "initial_generation",
        "invalid_generation_phase",
        "/proposed_tasks/phase",
        errors,
    );
    require(
        task.depends_on.is_empty(),
        "generation_must_not_have_dependencies",
        "/proposed_tasks/depends_on",
        errors,
    );
    validate_exact_set(
        &task.required_capabilities,
        &["stem_pedagogy", "structured_markdown", "basic_python"],
        "invalid_generation_capabilities",
        "/proposed_tasks/required_capabilities",
        errors,
    );
    validate_exact_set(
        &task.outputs,
        GENERATION_OUTPUTS,
        "invalid_generation_outputs",
        "/proposed_tasks/outputs",
        errors,
    );
    validate_exact_set(
        &task.validation_required,
        VALIDATION_CHECKS,
        "invalid_generation_validation_checks",
        "/proposed_tasks/validation_required",
        errors,
    );
    validate_exact_set(
        &task.human_review_required_for,
        &[PEER_REVIEWED],
        "missing_human_review_gate",
        "/proposed_tasks/human_review_required_for",
        errors,
    );
    require(
        task.execution_policy
            .as_deref()
            .is_none_or(|value| value == "code_generation_only"),
        "invalid_generation_execution_policy",
        "/proposed_tasks/execution_policy",
        errors,
    );
}

fn validate_validation_task(task: &ProposedTaskInput, errors: &mut Vec<GraphPolicyError>) {
    require(
        task.phase == "mechanical_validation",
        "invalid_validation_phase",
        "/proposed_tasks/phase",
        errors,
    );
    validate_exact_set(
        &task.required_capabilities,
        &["artifact_validation", "python_execution_limited"],
        "invalid_validation_capabilities",
        "/proposed_tasks/required_capabilities",
        errors,
    );
    validate_exact_set(
        &task.outputs,
        VALIDATION_OUTPUTS,
        "invalid_validation_outputs",
        "/proposed_tasks/outputs",
        errors,
    );
    require(
        task.validation_required.is_empty(),
        "validator_task_must_not_request_checks",
        "/proposed_tasks/validation_required",
        errors,
    );
    require(
        task.human_review_required_for.is_empty(),
        "validator_task_must_not_request_human_review",
        "/proposed_tasks/human_review_required_for",
        errors,
    );
    require(
        task.execution_policy.is_none(),
        "invalid_validation_execution_policy",
        "/proposed_tasks/execution_policy",
        errors,
    );
}

fn validate_review_task(task: &ProposedTaskInput, errors: &mut Vec<GraphPolicyError>) {
    require(
        task.phase == "human_review",
        "invalid_review_phase",
        "/proposed_tasks/phase",
        errors,
    );
    validate_exact_set(
        &task.required_capabilities,
        &["human_subject_review", "human_pedagogy_review"],
        "invalid_review_capabilities",
        "/proposed_tasks/required_capabilities",
        errors,
    );
    validate_exact_set(
        &task.outputs,
        REVIEW_OUTPUTS,
        "invalid_review_outputs",
        "/proposed_tasks/outputs",
        errors,
    );
    require(
        task.validation_required.is_empty(),
        "review_task_must_not_request_checks",
        "/proposed_tasks/validation_required",
        errors,
    );
    validate_exact_set(
        &task.human_review_required_for,
        &[PEER_REVIEWED],
        "missing_human_review_gate",
        "/proposed_tasks/human_review_required_for",
        errors,
    );
    require(
        task.execution_policy.is_none(),
        "invalid_review_execution_policy",
        "/proposed_tasks/execution_policy",
        errors,
    );
}

fn validate_graph_shape(
    by_id: &BTreeMap<&str, &ProposedTaskInput>,
    errors: &mut Vec<GraphPolicyError>,
) {
    let generation = by_id
        .values()
        .find(|task| task.task_type == "generate_lesson_pack")
        .map(|task| task.local_id.as_str());
    let validation = by_id
        .values()
        .find(|task| task.task_type == "run_artifact_validation")
        .map(|task| task.local_id.as_str());
    let review = by_id
        .values()
        .find(|task| task.task_type == "review_subject_and_pedagogy")
        .map(|task| task.local_id.as_str());
    let Some(generation_id) = generation else {
        require(false, "missing_generation_task", "/proposed_tasks", errors);
        return;
    };
    let Some(validation_id) = validation else {
        require(false, "missing_validation_task", "/proposed_tasks", errors);
        return;
    };
    let Some(review_id) = review else {
        require(
            false,
            "missing_human_review_task",
            "/proposed_tasks",
            errors,
        );
        return;
    };

    require(
        by_id
            .get(validation_id)
            .is_some_and(|task| task.depends_on == [generation_id]),
        "validation_must_depend_on_generation",
        "/proposed_tasks/depends_on",
        errors,
    );
    require(
        by_id
            .get(review_id)
            .is_some_and(|task| task.depends_on == [validation_id]),
        "review_must_depend_on_validation",
        "/proposed_tasks/depends_on",
        errors,
    );
    require(
        !has_cycle(by_id),
        "task_dependency_cycle",
        "/proposed_tasks/depends_on",
        errors,
    );
}

fn validate_promotion_preconditions(
    proposal: &ProposedTaskGraphRecord,
    context: &PromotionContext,
    ids: &PromotionIds,
) -> Result<(), GraphPolicyError> {
    if proposal.state != ProposedTaskGraphState::VerifiedForMvpPromotion {
        return Err(GraphPolicyError::new(
            "proposal_not_verified_for_promotion",
            "/proposal/state",
        ));
    }
    if proposal.request_id != context.request_id || proposal.scope_id != context.scope_id {
        return Err(GraphPolicyError::new(
            "promotion_lineage_mismatch",
            "/promotion/scope",
        ));
    }
    if !context.planning_task_completed_for_proposal {
        return Err(GraphPolicyError::new(
            "planning_task_not_completed_for_proposal",
            "/promotion/planning_task",
        ));
    }
    if !context.request_available_for_promotion {
        return Err(GraphPolicyError::new(
            "request_not_available_for_promotion",
            "/promotion/request",
        ));
    }
    if context.active_competing_planning_leases != 0 {
        return Err(GraphPolicyError::new(
            "active_competing_planning_lease",
            "/promotion/planning_leases",
        ));
    }
    if context.verification != Some(PlanVerificationOutcome::NoBlockingFindings) {
        return Err(GraphPolicyError::new(
            "accepted_plan_verification_required",
            "/promotion/verification",
        ));
    }
    if context.open_blocking_findings {
        return Err(GraphPolicyError::new(
            "blocking_finding_prevents_promotion",
            "/promotion/findings",
        ));
    }
    if proposal.central_risk_level != LOW_RISK {
        return Err(GraphPolicyError::new(
            "non_low_risk_not_promotable",
            "/proposal/central_risk_level",
        ));
    }
    if proposal.mvp_policy_fingerprint.as_deref() != Some(&mvp_policy_fingerprint(&proposal.tasks))
        || !normalized_tasks_match_mvp(&proposal.tasks)
    {
        return Err(GraphPolicyError::new(
            "proposal_policy_evidence_invalid",
            "/proposal/tasks",
        ));
    }
    if ids.work_packet_ids.len() != proposal.tasks.len() {
        return Err(GraphPolicyError::new(
            "incorrect_work_packet_id_count",
            "/promotion/work_packet_ids",
        ));
    }
    let unique_work_packet_ids: BTreeSet<&WorkPacketId> = ids.work_packet_ids.iter().collect();
    if unique_work_packet_ids.len() != ids.work_packet_ids.len() {
        return Err(GraphPolicyError::new(
            "duplicate_work_packet_id",
            "/promotion/work_packet_ids",
        ));
    }
    Ok(())
}

fn normalized_tasks_match_mvp(tasks: &[NormalizedTaskRecord]) -> bool {
    let by_type: BTreeMap<&str, &NormalizedTaskRecord> = tasks
        .iter()
        .map(|task| (task.task_type.as_str(), task))
        .collect();
    if tasks.len() != 3 || by_type.len() != 3 {
        return false;
    }
    let Some(generation) = by_type.get("generate_lesson_pack") else {
        return false;
    };
    let Some(validation) = by_type.get("run_artifact_validation") else {
        return false;
    };
    let Some(review) = by_type.get("review_subject_and_pedagogy") else {
        return false;
    };
    generation.phase == "initial_generation"
        && generation.depends_on.is_empty()
        && exact_str_set(
            &generation.required_capabilities,
            &["stem_pedagogy", "structured_markdown", "basic_python"],
        )
        && exact_str_set(&generation.outputs, GENERATION_OUTPUTS)
        && exact_str_set(&generation.validation_required, VALIDATION_CHECKS)
        && exact_str_set(&generation.human_review_required_for, &[PEER_REVIEWED])
        && generation
            .execution_policy
            .as_deref()
            .is_none_or(|value| value == "code_generation_only")
        && validation.phase == "mechanical_validation"
        && validation.depends_on == [generation.local_id.as_str()]
        && exact_str_set(
            &validation.required_capabilities,
            &["artifact_validation", "python_execution_limited"],
        )
        && exact_str_set(&validation.outputs, VALIDATION_OUTPUTS)
        && validation.validation_required.is_empty()
        && validation.human_review_required_for.is_empty()
        && validation.execution_policy.is_none()
        && review.phase == "human_review"
        && review.depends_on == [validation.local_id.as_str()]
        && exact_str_set(
            &review.required_capabilities,
            &["human_subject_review", "human_pedagogy_review"],
        )
        && exact_str_set(&review.outputs, REVIEW_OUTPUTS)
        && review.validation_required.is_empty()
        && exact_str_set(&review.human_review_required_for, &[PEER_REVIEWED])
        && review.execution_policy.is_none()
}

fn exact_str_set(actual: &[String], expected: &[&str]) -> bool {
    let actual_set: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
    let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
    actual.len() == expected.len() && actual_set == expected_set
}

fn materialize_work_packets(
    proposal: &ProposedTaskGraphRecord,
    ids: &PromotionIds,
) -> Result<Vec<WorkPacketRecord>, GraphPolicyError> {
    let id_by_local: BTreeMap<&str, WorkPacketId> = proposal
        .tasks
        .iter()
        .zip(ids.work_packet_ids.iter())
        .map(|(task, id)| (task.local_id.as_str(), id.clone()))
        .collect();

    proposal
        .tasks
        .iter()
        .zip(ids.work_packet_ids.iter())
        .map(|(task, work_packet_id)| {
            let depends_on_work_packet_ids = task
                .depends_on
                .iter()
                .map(|dependency| {
                    id_by_local
                        .get(dependency.as_str())
                        .cloned()
                        .ok_or_else(|| {
                            GraphPolicyError::new(
                                "promotion_dependency_rewrite_failed",
                                "/promotion/work_packets/depends_on",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(WorkPacketRecord {
                work_packet_id: work_packet_id.clone(),
                proposal_id: proposal.proposal_id.clone(),
                source_local_task_id: task.local_id.clone(),
                state: if depends_on_work_packet_ids.is_empty() {
                    WorkPacketState::Open
                } else {
                    WorkPacketState::BlockedByDependency
                },
                depends_on_work_packet_ids,
                snapshot: task.clone(),
            })
        })
        .collect()
}

fn validate_exact_set(
    actual: &[String],
    expected: &[&str],
    code: &'static str,
    field_path: &'static str,
    errors: &mut Vec<GraphPolicyError>,
) {
    let actual_set: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
    let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
    require(
        actual.len() == expected.len() && actual_set == expected_set,
        code,
        field_path,
        errors,
    );
}

fn valid_local_id(local_id: &str) -> bool {
    !local_id.is_empty()
        && local_id.len() <= 64
        && !local_id.starts_with("system_")
        && !local_id.starts_with("admin_")
        && !local_id.starts_with("credential_")
        && local_id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        })
}

fn has_cycle(by_id: &BTreeMap<&str, &ProposedTaskInput>) -> bool {
    fn visit<'a>(
        id: &'a str,
        by_id: &BTreeMap<&'a str, &'a ProposedTaskInput>,
        visiting: &mut BTreeSet<&'a str>,
        visited: &mut BTreeSet<&'a str>,
    ) -> bool {
        if visited.contains(id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        if let Some(task) = by_id.get(id) {
            for dependency in &task.depends_on {
                if by_id.contains_key(dependency.as_str())
                    && visit(dependency, by_id, visiting, visited)
                {
                    return true;
                }
            }
        }
        visiting.remove(id);
        visited.insert(id);
        false
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    by_id
        .keys()
        .any(|id| visit(id, by_id, &mut visiting, &mut visited))
}

fn require(
    condition: bool,
    code: &'static str,
    field_path: &'static str,
    errors: &mut Vec<GraphPolicyError>,
) {
    if !condition {
        errors.push(GraphPolicyError::new(code, field_path));
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedTaskGraphInput {
    proposal_id: String,
    request_id: String,
    planning_task_id: String,
    planner_runner_id: String,
    schema_version: String,
    status: String,
    source_request_summary: SourceRequestSummaryInput,
    assumptions: Vec<String>,
    missing_information: Vec<String>,
    proposed_artifacts: Vec<ProposedArtifactInput>,
    proposed_tasks: Vec<ProposedTaskInput>,
    validation_plan: Vec<String>,
    human_review_required_for: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRequestSummaryInput {
    subject: String,
    topic: String,
    age_range: String,
    duration_minutes: u16,
    language: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedArtifactInput {
    artifact_type: String,
    priority: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedTaskInput {
    local_id: String,
    phase: String,
    task_type: String,
    subject: String,
    topic: String,
    age_range: String,
    language: String,
    risk_level: String,
    #[serde(default)]
    depends_on: Vec<String>,
    required_capabilities: Vec<String>,
    outputs: Vec<String>,
    validation_required: Vec<String>,
    human_review_required_for: Vec<String>,
    execution_policy: Option<String>,
}

impl From<&ProposedTaskInput> for NormalizedTaskRecord {
    fn from(value: &ProposedTaskInput) -> Self {
        Self {
            local_id: value.local_id.clone(),
            phase: value.phase.clone(),
            task_type: value.task_type.clone(),
            required_capabilities: value.required_capabilities.clone(),
            outputs: value.outputs.clone(),
            depends_on: value.depends_on.clone(),
            validation_required: value.validation_required.clone(),
            human_review_required_for: value.human_review_required_for.clone(),
            execution_policy: value.execution_policy.clone(),
        }
    }
}

impl From<crate::error::IdError> for GraphPolicyError {
    fn from(_: crate::error::IdError) -> Self {
        Self::new("invalid_submitted_identifier", "/")
    }
}
