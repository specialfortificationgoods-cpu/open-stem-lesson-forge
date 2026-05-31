use crate::ids::{PlanningTaskId, RequestId};
use crate::state::{ActorCapability, PlanningTaskState, TrustLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningClaimPolicy {
    pub lease_minutes: u16,
    pub heartbeat_interval_seconds: u16,
    pub heartbeat_grace_seconds: u16,
    pub max_active_claims: u16,
    pub min_accepted_proposals: u16,
    pub allow_duplicate_claims: bool,
    pub replacement_claim_after_seconds: u16,
    pub max_claim_attempts: u16,
}

impl PlanningClaimPolicy {
    pub fn mvp_default() -> Self {
        Self {
            lease_minutes: 60,
            heartbeat_interval_seconds: 60,
            heartbeat_grace_seconds: 180,
            max_active_claims: 2,
            min_accepted_proposals: 1,
            allow_duplicate_claims: true,
            replacement_claim_after_seconds: 300,
            max_claim_attempts: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningTaskRecord {
    pub planning_task_id: PlanningTaskId,
    pub request_id: RequestId,
    pub scope_id: String,
    pub task_type: &'static str,
    pub phase: &'static str,
    pub input_refs: Vec<String>,
    pub required_output_schema: &'static str,
    pub allowed_outputs: Vec<&'static str>,
    pub forbidden_outputs: Vec<&'static str>,
    pub required_capabilities: Vec<ActorCapability>,
    pub minimum_runner_trust_level: TrustLevel,
    pub claim_policy: PlanningClaimPolicy,
    pub state: PlanningTaskState,
}

pub fn mechanical_planning_task(
    planning_task_id: PlanningTaskId,
    request_id: RequestId,
    scope_id: String,
) -> PlanningTaskRecord {
    PlanningTaskRecord {
        planning_task_id,
        input_refs: vec![request_id.to_string()],
        request_id,
        scope_id,
        task_type: "propose_task_graph",
        phase: "request_normalization",
        required_output_schema: "proposed_task_graph.schema.json",
        allowed_outputs: vec!["proposed_task_graph"],
        forbidden_outputs: vec![
            "arbitrary_prompt",
            "student_grading_task",
            "credential_handling_task",
        ],
        required_capabilities: vec![
            ActorCapability::RequestInterpretation,
            ActorCapability::TaskDecomposition,
            ActorCapability::StructuredJsonOutput,
            ActorCapability::PolicyReasoning,
        ],
        minimum_runner_trust_level: TrustLevel::PlannerCandidate,
        claim_policy: PlanningClaimPolicy::mvp_default(),
        state: PlanningTaskState::Open,
    }
}
