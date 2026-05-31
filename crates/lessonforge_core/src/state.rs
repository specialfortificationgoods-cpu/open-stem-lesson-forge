use crate::error::{LeaseError, TransitionError};
use crate::ids::{ActorId, LeaseId};
use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value,)+
                }
            }
        }
    };
}

fn transition_error(
    entity: &'static str,
    from: &'static str,
    action: &'static str,
) -> TransitionError {
    TransitionError {
        entity,
        from,
        action,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    PublicRequester,
    AuthenticatedRequester,
    Runner,
    VerifierRunner,
    HumanReviewer,
    RunnerOperator,
    Curator,
    Admin,
    SystemCore,
    SystemValidator,
}

string_enum!(ActorType {
    PublicRequester => "public_requester",
    AuthenticatedRequester => "authenticated_requester",
    Runner => "runner",
    VerifierRunner => "verifier_runner",
    HumanReviewer => "human_reviewer",
    RunnerOperator => "runner_operator",
    Curator => "curator",
    Admin => "admin",
    SystemCore => "system_core",
    SystemValidator => "system_validator",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorCapability {
    RequestInterpretation,
    ContentModeration,
    AgeAppropriatenessClassification,
    StructuredJsonOutput,
    TaskDecomposition,
    PolicyReasoning,
    PolicyCrossCheck,
    StemPedagogy,
    StructuredMarkdown,
    BasicPython,
    PlanConsistencyReview,
    ArtifactGeneration,
    ArtifactValidation,
    PythonExecutionLimited,
    HumanSubjectReview,
    HumanPedagogyReview,
}

string_enum!(ActorCapability {
    RequestInterpretation => "request_interpretation",
    ContentModeration => "content_moderation",
    AgeAppropriatenessClassification => "age_appropriateness_classification",
    StructuredJsonOutput => "structured_json_output",
    TaskDecomposition => "task_decomposition",
    PolicyReasoning => "policy_reasoning",
    PolicyCrossCheck => "policy_cross_check",
    StemPedagogy => "stem_pedagogy",
    StructuredMarkdown => "structured_markdown",
    BasicPython => "basic_python",
    PlanConsistencyReview => "plan_consistency_review",
    ArtifactGeneration => "artifact_generation",
    ArtifactValidation => "artifact_validation",
    PythonExecutionLimited => "python_execution_limited",
    HumanSubjectReview => "human_subject_review",
    HumanPedagogyReview => "human_pedagogy_review",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Untrusted,
    RunnerCandidate,
    ModerationCandidate,
    PlannerCandidate,
    VerifierCandidate,
    ReviewerCandidate,
    ReviewerApproved,
    Curator,
    Admin,
    System,
}

string_enum!(TrustLevel {
    Untrusted => "untrusted",
    RunnerCandidate => "runner_candidate",
    ModerationCandidate => "moderation_candidate",
    PlannerCandidate => "planner_candidate",
    VerifierCandidate => "verifier_candidate",
    ReviewerCandidate => "reviewer_candidate",
    ReviewerApproved => "reviewer_approved",
    Curator => "curator",
    Admin => "admin",
    System => "system",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorStatus {
    Active,
    Paused,
    Revoked,
}

string_enum!(ActorStatus {
    Active => "active",
    Paused => "paused",
    Revoked => "revoked",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestState {
    Requested,
    ModerationPending,
    ModerationPassed,
    Rejected,
    Quarantined,
    PlanningOpen,
    PlanningInProgress,
    PlanningFailed,
    PlanProposed,
    Decomposed,
    ArtifactDrafted,
    MachineValidated,
    PeerReviewed,
    Deprecated,
}

string_enum!(RequestState {
    Requested => "requested",
    ModerationPending => "moderation_pending",
    ModerationPassed => "moderation_passed",
    Rejected => "rejected",
    Quarantined => "quarantined",
    PlanningOpen => "planning_open",
    PlanningInProgress => "planning_in_progress",
    PlanningFailed => "planning_failed",
    PlanProposed => "plan_proposed",
    Decomposed => "decomposed",
    ArtifactDrafted => "artifact_drafted",
    MachineValidated => "machine_validated",
    PeerReviewed => "peer_reviewed",
    Deprecated => "deprecated",
});

impl RequestState {
    pub fn transition(self, action: RequestTransition) -> Result<Self, TransitionError> {
        match (self, action) {
            (Self::Requested, RequestTransition::DeterministicIntakeRejected) => Ok(Self::Rejected),
            (Self::Requested, RequestTransition::DeterministicQuarantine) => Ok(Self::Quarantined),
            (Self::Requested, RequestTransition::DeterministicIntakePassed) => {
                Ok(Self::ModerationPending)
            }
            (Self::ModerationPending, RequestTransition::ModerationRejects) => Ok(Self::Rejected),
            (Self::ModerationPending, RequestTransition::ModerationQuarantines) => {
                Ok(Self::Quarantined)
            }
            (Self::ModerationPending, RequestTransition::ModerationAllowsPlanning) => {
                Ok(Self::ModerationPassed)
            }
            (Self::ModerationPassed, RequestTransition::CreatePlanningTask) => {
                Ok(Self::PlanningOpen)
            }
            (Self::PlanningOpen, RequestTransition::PlanningLeaseActive) => {
                Ok(Self::PlanningInProgress)
            }
            (Self::PlanningOpen | Self::PlanningInProgress, RequestTransition::PlanningFailed) => {
                Ok(Self::PlanningFailed)
            }
            (Self::PlanningInProgress, RequestTransition::ProposalStored) => Ok(Self::PlanProposed),
            (Self::PlanProposed, RequestTransition::PromotionAccepted) => Ok(Self::Decomposed),
            (Self::Decomposed, RequestTransition::ArtifactBundleAccepted) => {
                Ok(Self::ArtifactDrafted)
            }
            (Self::ArtifactDrafted, RequestTransition::TrustedValidationPassed) => {
                Ok(Self::MachineValidated)
            }
            (Self::MachineValidated, RequestTransition::HumanReviewApproved) => {
                Ok(Self::PeerReviewed)
            }
            (
                Self::Requested
                | Self::ModerationPending
                | Self::ModerationPassed
                | Self::PlanningOpen
                | Self::PlanningInProgress
                | Self::PlanningFailed
                | Self::PlanProposed
                | Self::Decomposed
                | Self::ArtifactDrafted
                | Self::MachineValidated,
                RequestTransition::Deprecate,
            ) => Ok(Self::Deprecated),
            _ => Err(transition_error("request", self.as_str(), action.as_str())),
        }
    }

    pub fn apply_transition(
        self,
        action: RequestTransition,
        context: TransitionContext,
    ) -> Result<AppliedTransition<Self>, TransitionError> {
        let previous_state = self.as_str();
        let next_state = self.transition(action)?;
        Ok(AppliedTransition {
            next_state,
            event: StateTransitionEvent {
                event_id: context.event_id,
                entity_type: context.entity_type,
                entity_id: context.entity_id,
                scope_id: context.scope_id,
                actor_id: context.actor_id,
                actor_type: context.actor_type,
                command_id: context.command_id,
                action: action.as_str().to_owned(),
                reason_code: context.reason_code,
                safe_field_path: context.safe_field_path,
                related_ids: context.related_ids,
                occurred_at: context.occurred_at,
                previous_state,
                next_state: next_state.as_str(),
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestTransition {
    DeterministicIntakeRejected,
    DeterministicQuarantine,
    DeterministicIntakePassed,
    ModerationRejects,
    ModerationQuarantines,
    ModerationAllowsPlanning,
    CreatePlanningTask,
    PlanningLeaseActive,
    PlanningFailed,
    ProposalStored,
    PromotionAccepted,
    ArtifactBundleAccepted,
    TrustedValidationPassed,
    HumanReviewApproved,
    Deprecate,
}

string_enum!(RequestTransition {
    DeterministicIntakeRejected => "deterministic_intake_rejected",
    DeterministicQuarantine => "deterministic_quarantine",
    DeterministicIntakePassed => "deterministic_intake_passed",
    ModerationRejects => "moderation_rejects",
    ModerationQuarantines => "moderation_quarantines",
    ModerationAllowsPlanning => "moderation_allows_planning",
    CreatePlanningTask => "create_planning_task",
    PlanningLeaseActive => "planning_lease_active",
    PlanningFailed => "planning_failed",
    ProposalStored => "proposal_stored",
    PromotionAccepted => "promotion_accepted",
    ArtifactBundleAccepted => "artifact_bundle_accepted",
    TrustedValidationPassed => "trusted_validation_passed",
    HumanReviewApproved => "human_review_approved",
    Deprecate => "deprecate",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestModerationTaskState {
    Open,
    Claimed,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningTaskState {
    Open,
    Claimed,
    Submitted,
    Completed,
    Cancelled,
}

string_enum!(PlanningTaskState {
    Open => "open",
    Claimed => "claimed",
    Submitted => "submitted",
    Completed => "completed",
    Cancelled => "cancelled",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningTaskTransition {
    Claim,
    Submit,
    Complete,
    LeaseExpiredOrReleased,
    Cancel,
}

string_enum!(PlanningTaskTransition {
    Claim => "claim",
    Submit => "submit",
    Complete => "complete",
    LeaseExpiredOrReleased => "lease_expired_or_released",
    Cancel => "cancel",
});

impl PlanningTaskState {
    pub fn transition(self, action: PlanningTaskTransition) -> Result<Self, TransitionError> {
        match (self, action) {
            (Self::Open, PlanningTaskTransition::Claim) => Ok(Self::Claimed),
            (Self::Claimed, PlanningTaskTransition::Claim) => Ok(Self::Claimed),
            (Self::Claimed, PlanningTaskTransition::Submit) => Ok(Self::Submitted),
            (Self::Submitted, PlanningTaskTransition::Complete) => Ok(Self::Completed),
            (Self::Open | Self::Claimed, PlanningTaskTransition::Cancel) => Ok(Self::Cancelled),
            _ => Err(transition_error(
                "planning_task",
                self.as_str(),
                action.as_str(),
            )),
        }
    }

    pub fn after_lease_end(self, facts: PlanningLeaseFacts) -> Result<Self, TransitionError> {
        if self != Self::Claimed {
            return Err(transition_error(
                "planning_task",
                self.as_str(),
                "lease_end_reduction",
            ));
        }
        if facts.valid_proposal_submitted {
            return Ok(Self::Submitted);
        }
        if facts.active_lease_count > 0 {
            return Ok(Self::Claimed);
        }
        if facts.retry_allowed {
            return Ok(Self::Open);
        }
        Ok(Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanningLeaseFacts {
    pub active_lease_count: u32,
    pub valid_proposal_submitted: bool,
    pub retry_allowed: bool,
}

macro_rules! claimable_state {
    ($name:ident, $entity:literal) => {
        string_enum!($name {
            Open => "open",
            Claimed => "claimed",
            Submitted => "submitted",
            Completed => "completed",
            Cancelled => "cancelled",
        });

        impl $name {
            pub fn transition(
                self,
                action: PlanningTaskTransition,
            ) -> Result<Self, TransitionError> {
                match (self, action) {
                    (Self::Open, PlanningTaskTransition::Claim) => Ok(Self::Claimed),
                    (Self::Claimed, PlanningTaskTransition::Submit) => Ok(Self::Submitted),
                    (Self::Submitted, PlanningTaskTransition::Complete) => Ok(Self::Completed),
                    (Self::Open | Self::Claimed, PlanningTaskTransition::Cancel) => {
                        Ok(Self::Cancelled)
                    }
                    _ => Err(transition_error($entity, self.as_str(), action.as_str())),
                }
            }
        }
    };
}

string_enum!(RequestModerationTaskState {
    Open => "open",
    Claimed => "claimed",
    Completed => "completed",
    Cancelled => "cancelled",
});

impl RequestModerationTaskState {
    pub fn transition(self, action: PlanningTaskTransition) -> Result<Self, TransitionError> {
        match (self, action) {
            (Self::Open, PlanningTaskTransition::Claim) => Ok(Self::Claimed),
            (Self::Claimed, PlanningTaskTransition::Complete) => Ok(Self::Completed),
            (Self::Open | Self::Claimed, PlanningTaskTransition::Cancel) => Ok(Self::Cancelled),
            _ => Err(transition_error(
                "request_moderation_task",
                self.as_str(),
                action.as_str(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanVerificationTaskState {
    Open,
    Claimed,
    Submitted,
    Completed,
    Cancelled,
}

claimable_state!(PlanVerificationTaskState, "plan_verification_task");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTaskState {
    Open,
    Claimed,
    Submitted,
    Completed,
    Cancelled,
}

claimable_state!(ReviewTaskState, "review_task");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposedTaskGraphState {
    Proposed,
    SchemaRejected,
    PolicyRejected,
    SchemaPolicyValidated,
    VerificationRequired,
    VerificationBlocked,
    VerifiedForMvpPromotion,
    PromotionRejected,
    Promoted,
    Superseded,
    Quarantined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposedTaskGraphTransition {
    SchemaRejected,
    PolicyRejected,
    SchemaPolicyValidated,
    RequireVerification,
    VerificationBlocked,
    VerifiedForMvpPromotion,
    PromotionRejected,
    Promote,
    Supersede,
    Quarantine,
}

string_enum!(ProposedTaskGraphState {
    Proposed => "proposed",
    SchemaRejected => "schema_rejected",
    PolicyRejected => "policy_rejected",
    SchemaPolicyValidated => "schema_policy_validated",
    VerificationRequired => "verification_required",
    VerificationBlocked => "verification_blocked",
    VerifiedForMvpPromotion => "verified_for_mvp_promotion",
    PromotionRejected => "promotion_rejected",
    Promoted => "promoted",
    Superseded => "superseded",
    Quarantined => "quarantined",
});

string_enum!(ProposedTaskGraphTransition {
    SchemaRejected => "schema_rejected",
    PolicyRejected => "policy_rejected",
    SchemaPolicyValidated => "schema_policy_validated",
    RequireVerification => "require_verification",
    VerificationBlocked => "verification_blocked",
    VerifiedForMvpPromotion => "verified_for_mvp_promotion",
    PromotionRejected => "promotion_rejected",
    Promote => "promote",
    Supersede => "supersede",
    Quarantine => "quarantine",
});

impl ProposedTaskGraphState {
    pub fn transition(self, action: ProposedTaskGraphTransition) -> Result<Self, TransitionError> {
        match (self, action) {
            (Self::Proposed, ProposedTaskGraphTransition::SchemaRejected) => {
                Ok(Self::SchemaRejected)
            }
            (Self::Proposed, ProposedTaskGraphTransition::PolicyRejected) => {
                Ok(Self::PolicyRejected)
            }
            (Self::Proposed, ProposedTaskGraphTransition::SchemaPolicyValidated) => {
                Ok(Self::SchemaPolicyValidated)
            }
            (Self::SchemaPolicyValidated, ProposedTaskGraphTransition::RequireVerification) => {
                Ok(Self::VerificationRequired)
            }
            (Self::VerificationRequired, ProposedTaskGraphTransition::VerificationBlocked) => {
                Ok(Self::VerificationBlocked)
            }
            (Self::VerificationRequired, ProposedTaskGraphTransition::VerifiedForMvpPromotion) => {
                Ok(Self::VerifiedForMvpPromotion)
            }
            (Self::VerifiedForMvpPromotion, ProposedTaskGraphTransition::Promote) => {
                Ok(Self::Promoted)
            }
            (
                Self::Proposed
                | Self::SchemaPolicyValidated
                | Self::VerificationRequired
                | Self::VerificationBlocked
                | Self::VerifiedForMvpPromotion,
                ProposedTaskGraphTransition::PromotionRejected,
            ) => Ok(Self::PromotionRejected),
            (
                Self::Proposed
                | Self::SchemaPolicyValidated
                | Self::VerificationRequired
                | Self::VerificationBlocked
                | Self::VerifiedForMvpPromotion
                | Self::PromotionRejected
                | Self::Promoted,
                ProposedTaskGraphTransition::Supersede,
            ) => Ok(Self::Superseded),
            (
                Self::Proposed
                | Self::SchemaPolicyValidated
                | Self::VerificationRequired
                | Self::VerificationBlocked
                | Self::VerifiedForMvpPromotion
                | Self::PromotionRejected,
                ProposedTaskGraphTransition::Quarantine,
            ) => Ok(Self::Quarantined),
            _ => Err(transition_error(
                "proposed_task_graph",
                self.as_str(),
                action.as_str(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkPacketState {
    BlockedByDependency,
    Open,
    Claimed,
    Submitted,
    Interrupted,
    Accepted,
    Rejected,
    Cancelled,
}

string_enum!(WorkPacketState {
    BlockedByDependency => "blocked_by_dependency",
    Open => "open",
    Claimed => "claimed",
    Submitted => "submitted",
    Interrupted => "interrupted",
    Accepted => "accepted",
    Rejected => "rejected",
    Cancelled => "cancelled",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkPacketTransition {
    DependenciesSatisfied,
    Claim,
    Submit,
    AcceptSubmission,
    RejectSubmission,
    AcceptInterruption,
    Cancel,
}

string_enum!(WorkPacketTransition {
    DependenciesSatisfied => "dependencies_satisfied",
    Claim => "claim",
    Submit => "submit",
    AcceptSubmission => "accept_submission",
    RejectSubmission => "reject_submission",
    AcceptInterruption => "accept_interruption",
    Cancel => "cancel",
});

impl WorkPacketState {
    pub fn transition(self, action: WorkPacketTransition) -> Result<Self, TransitionError> {
        match (self, action) {
            (Self::BlockedByDependency, WorkPacketTransition::DependenciesSatisfied) => {
                Ok(Self::Open)
            }
            (Self::Open, WorkPacketTransition::Claim) => Ok(Self::Claimed),
            (Self::Claimed, WorkPacketTransition::Submit) => Ok(Self::Submitted),
            (Self::Submitted, WorkPacketTransition::AcceptSubmission) => Ok(Self::Accepted),
            (Self::Submitted, WorkPacketTransition::RejectSubmission) => Ok(Self::Rejected),
            (Self::Submitted, WorkPacketTransition::AcceptInterruption) => Ok(Self::Interrupted),
            (
                Self::BlockedByDependency
                | Self::Open
                | Self::Claimed
                | Self::Submitted
                | Self::Interrupted,
                WorkPacketTransition::Cancel,
            ) => Ok(Self::Cancelled),
            _ => Err(transition_error(
                "work_packet",
                self.as_str(),
                action.as_str(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactState {
    DraftGenerated,
    ValidationFailed,
    MachineValidated,
    ReviewRequested,
    PeerReviewed,
    Quarantined,
    Deprecated,
}

string_enum!(ArtifactState {
    DraftGenerated => "draft_generated",
    ValidationFailed => "validation_failed",
    MachineValidated => "machine_validated",
    ReviewRequested => "review_requested",
    PeerReviewed => "peer_reviewed",
    Quarantined => "quarantined",
    Deprecated => "deprecated",
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionContext {
    pub event_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub scope_id: String,
    pub actor_id: ActorId,
    pub actor_type: ActorType,
    pub command_id: String,
    pub action: String,
    pub reason_code: String,
    pub safe_field_path: Option<String>,
    pub related_ids: Vec<String>,
    pub occurred_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransitionEvent {
    pub event_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub scope_id: String,
    pub actor_id: ActorId,
    pub actor_type: ActorType,
    pub command_id: String,
    pub action: String,
    pub reason_code: String,
    pub safe_field_path: Option<String>,
    pub related_ids: Vec<String>,
    pub occurred_at: u64,
    pub previous_state: &'static str,
    pub next_state: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedTransition<T> {
    pub next_state: T,
    pub event: StateTransitionEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseEntityType {
    RequestModerationTask,
    PlanningTask,
    PlanVerificationTask,
    WorkPacket,
    ReviewTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseState {
    Active,
    Consumed,
    Released,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseSubmission {
    pub actor_id: ActorId,
    pub entity_type: LeaseEntityType,
    pub entity_id: String,
    pub claim_token: String,
    pub now: u64,
    pub idempotency_key: String,
    pub payload_digest: String,
    pub result_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseMutationCommand {
    pub actor_id: ActorId,
    pub claim_token: String,
    pub now: u64,
    pub idempotency_key: String,
    pub payload_digest: String,
    pub result_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseSubmissionReplay {
    pub actor_id: ActorId,
    pub entity_type: LeaseEntityType,
    pub entity_id: String,
    pub claim_token: String,
    pub idempotency_key: String,
    pub payload_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseAction {
    Submit(LeaseSubmission),
    Heartbeat(LeaseMutationCommand),
    Release(LeaseMutationCommand),
    Revoke(LeaseMutationCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseReplayDecision {
    ReturnOriginalResult { result_id: String },
    RejectChangedReplay,
    NotReplay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IdempotencyRecord {
    key: String,
    payload_digest: String,
    result_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    lease_id: LeaseId,
    entity_type: LeaseEntityType,
    entity_id: String,
    actor_id: ActorId,
    lease_slot: Option<u32>,
    claim_token_hash: String,
    claimed_at: u64,
    expires_at: u64,
    state: LeaseState,
    last_heartbeat_at: Option<u64>,
    submission: Option<IdempotencyRecord>,
    mutation_records: Vec<IdempotencyRecord>,
}

impl Lease {
    pub fn active(
        lease_id: LeaseId,
        entity_type: LeaseEntityType,
        entity_id: String,
        actor_id: ActorId,
        claim_token_hash: String,
        claimed_at: u64,
        expires_at: u64,
    ) -> Self {
        Self::active_with_slot(
            lease_id,
            entity_type,
            entity_id,
            actor_id,
            None,
            claim_token_hash,
            claimed_at,
            expires_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn active_with_slot(
        lease_id: LeaseId,
        entity_type: LeaseEntityType,
        entity_id: String,
        actor_id: ActorId,
        lease_slot: Option<u32>,
        claim_token_hash: String,
        claimed_at: u64,
        expires_at: u64,
    ) -> Self {
        Self {
            lease_id,
            entity_type,
            entity_id,
            actor_id,
            lease_slot,
            claim_token_hash,
            claimed_at,
            expires_at,
            state: LeaseState::Active,
            last_heartbeat_at: None,
            submission: None,
            mutation_records: Vec::new(),
        }
    }

    pub fn claim_token_hash(claim_token: &str) -> String {
        use sha2::{Digest, Sha256};
        format!("sha256:{:x}", Sha256::digest(claim_token.as_bytes()))
    }

    pub fn state(&self) -> LeaseState {
        self.state
    }

    pub fn effective_state(&self, now: u64) -> LeaseState {
        if self.state == LeaseState::Active && now >= self.expires_at {
            LeaseState::Expired
        } else {
            self.state
        }
    }

    pub fn lease_slot(&self) -> Option<u32> {
        self.lease_slot
    }

    pub fn last_heartbeat_at(&self) -> Option<u64> {
        self.last_heartbeat_at
    }

    pub fn replay_submission(
        &self,
        replay: LeaseSubmissionReplay,
    ) -> Result<LeaseReplayDecision, LeaseError> {
        self.require_actor(&replay.actor_id)?;
        if self.entity_type != replay.entity_type || self.entity_id != replay.entity_id {
            return Err(LeaseError::EntityMismatch);
        }
        self.require_token(&replay.claim_token)?;
        let Some(record) = &self.submission else {
            return Ok(LeaseReplayDecision::NotReplay);
        };
        if record.key != replay.idempotency_key {
            return Ok(LeaseReplayDecision::NotReplay);
        }
        if record.payload_digest == replay.payload_digest {
            return Ok(LeaseReplayDecision::ReturnOriginalResult {
                result_id: record.result_id.clone(),
            });
        }
        Ok(LeaseReplayDecision::RejectChangedReplay)
    }

    pub fn apply(self, action: LeaseAction) -> Result<Self, LeaseError> {
        match action {
            LeaseAction::Submit(submission) => self.apply_submission(submission),
            LeaseAction::Heartbeat(command) => self.apply_heartbeat(command),
            LeaseAction::Release(command) => self.apply_release(command),
            LeaseAction::Revoke(command) => self.apply_revoke(command),
        }
    }

    fn apply_submission(mut self, submission: LeaseSubmission) -> Result<Self, LeaseError> {
        self.require_active()?;
        self.require_actor(&submission.actor_id)?;
        if self.entity_type != submission.entity_type || self.entity_id != submission.entity_id {
            return Err(LeaseError::EntityMismatch);
        }
        self.require_time_and_token(submission.now, &submission.claim_token)?;
        if submission.idempotency_key.is_empty()
            || submission.payload_digest.is_empty()
            || submission.result_id.is_empty()
        {
            return Err(LeaseError::MissingSubmissionRecord);
        }
        self.submission = Some(IdempotencyRecord {
            key: submission.idempotency_key,
            payload_digest: submission.payload_digest,
            result_id: submission.result_id,
        });
        self.state = LeaseState::Consumed;
        Ok(self)
    }

    fn apply_heartbeat(mut self, command: LeaseMutationCommand) -> Result<Self, LeaseError> {
        self.require_actor(&command.actor_id)?;
        self.require_token(&command.claim_token)?;
        Self::require_command_record(&command)?;
        if self.is_replayed_mutation(&command)? {
            return Ok(self);
        }
        self.require_not_expired(command.now)?;
        self.require_active()?;
        self.last_heartbeat_at = Some(command.now);
        self.record_mutation(command);
        Ok(self)
    }

    fn apply_release(mut self, command: LeaseMutationCommand) -> Result<Self, LeaseError> {
        self.require_actor(&command.actor_id)?;
        self.require_token(&command.claim_token)?;
        Self::require_command_record(&command)?;
        if self.is_replayed_mutation(&command)? {
            return Ok(self);
        }
        self.require_not_expired(command.now)?;
        self.require_active()?;
        self.record_mutation(command);
        self.state = LeaseState::Released;
        Ok(self)
    }

    fn apply_revoke(mut self, command: LeaseMutationCommand) -> Result<Self, LeaseError> {
        self.require_actor(&command.actor_id)?;
        self.require_token(&command.claim_token)?;
        Self::require_command_record(&command)?;
        if self.is_replayed_mutation(&command)? {
            return Ok(self);
        }
        self.require_not_expired(command.now)?;
        self.require_active()?;
        self.record_mutation(command);
        self.state = LeaseState::Revoked;
        Ok(self)
    }

    fn require_active(&self) -> Result<(), LeaseError> {
        if self.state == LeaseState::Active {
            return Ok(());
        }
        Err(LeaseError::InvalidState)
    }

    fn require_actor(&self, actor_id: &ActorId) -> Result<(), LeaseError> {
        if &self.actor_id == actor_id {
            return Ok(());
        }
        Err(LeaseError::ActorMismatch)
    }

    fn require_time_and_token(&self, now: u64, claim_token: &str) -> Result<(), LeaseError> {
        self.require_not_expired(now)?;
        self.require_token(claim_token)
    }

    fn require_not_expired(&self, now: u64) -> Result<(), LeaseError> {
        if self.effective_state(now) == LeaseState::Expired {
            return Err(LeaseError::Expired);
        }
        Ok(())
    }

    fn require_token(&self, claim_token: &str) -> Result<(), LeaseError> {
        if Self::claim_token_hash(claim_token) != self.claim_token_hash {
            return Err(LeaseError::ClaimTokenMismatch);
        }
        Ok(())
    }

    fn is_replayed_mutation(&self, command: &LeaseMutationCommand) -> Result<bool, LeaseError> {
        if let Some(record) = self
            .mutation_records
            .iter()
            .find(|record| record.key == command.idempotency_key)
        {
            if record.payload_digest == command.payload_digest {
                return Ok(true);
            }
            return Err(LeaseError::IdempotencyConflict);
        }
        Ok(false)
    }

    fn record_mutation(&mut self, command: LeaseMutationCommand) {
        self.mutation_records.push(IdempotencyRecord {
            key: command.idempotency_key,
            payload_digest: command.payload_digest,
            result_id: command.result_id,
        });
    }

    fn require_command_record(command: &LeaseMutationCommand) -> Result<(), LeaseError> {
        if command.idempotency_key.is_empty()
            || command.payload_digest.is_empty()
            || command.result_id.is_empty()
        {
            return Err(LeaseError::MissingSubmissionRecord);
        }
        Ok(())
    }
}
