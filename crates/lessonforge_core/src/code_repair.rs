use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

pub use crate::state::{ActorType, ArtifactState, WorkPacketState, WorkPacketTransition};

impl ActorType {
    fn can_issue_repair_continuation(self) -> bool {
        matches!(
            self,
            ActorType::RunnerOperator | ActorType::Curator | ActorType::Admin
        )
    }
}

impl ArtifactState {
    fn eligible_for_automated_repair(self) -> bool {
        matches!(
            self,
            ArtifactState::DraftGenerated | ArtifactState::ValidationFailed
        )
    }

    fn blocks_repair_continuation(self) -> bool {
        matches!(
            self,
            ArtifactState::Quarantined
                | ArtifactState::Deprecated
                | ArtifactState::ReviewRequested
                | ArtifactState::PeerReviewed
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomatedRepairPreference {
    NoAutomatedRepair,
    RequestBoundedCodeRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomatedRepairDeclineReason {
    NotRequested,
    RequestNotEligible,
    ModerationNotAllowed,
    ArtifactStateNotEligible,
    NonCodeSafetyChecksFailed,
    QuarantineRequired,
    HumanReviewAlreadyOpen,
    AttemptLimitReached,
    NoOptedInRunnerAvailable,
    RunnerScopeOrCapabilityMismatch,
    CuratorOrAdminDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomatedRepairLoopStatus {
    NotRequested,
    DeclinedByPolicy {
        reason: AutomatedRepairDeclineReason,
    },
    WaitingForOptedInRunner,
    WaitingForOperatorReview,
    NeedsOperatorReview,
    Running,
    ExhaustedAttempts,
    CompletedRepairedDraftCreated,
    StoppedForQuarantine,
    StoppedForHumanReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomatedRepairInputs {
    pub request_preference: AutomatedRepairPreference,
    pub request_eligible: bool,
    pub moderation_allowed: bool,
    pub artifact_state: ArtifactState,
    pub required_non_code_safety_checks_passed: bool,
    pub quarantine_required: bool,
    pub human_review_open: bool,
    pub repair_attempt_limit_reached: bool,
    pub opted_in_runner_available: bool,
    pub runner_scope_and_capability_match: bool,
    pub curator_or_admin_disabled: bool,
}

impl AutomatedRepairInputs {
    pub fn evaluate(self) -> AutomatedRepairLoopStatus {
        if self.request_preference == AutomatedRepairPreference::NoAutomatedRepair {
            return AutomatedRepairLoopStatus::NotRequested;
        }

        if !self.request_eligible {
            return declined(AutomatedRepairDeclineReason::RequestNotEligible);
        }
        if !self.moderation_allowed {
            return declined(AutomatedRepairDeclineReason::ModerationNotAllowed);
        }
        if !self.artifact_state.eligible_for_automated_repair() {
            return declined(AutomatedRepairDeclineReason::ArtifactStateNotEligible);
        }
        if !self.required_non_code_safety_checks_passed {
            return declined(AutomatedRepairDeclineReason::NonCodeSafetyChecksFailed);
        }
        if self.quarantine_required {
            return declined(AutomatedRepairDeclineReason::QuarantineRequired);
        }
        if self.human_review_open {
            return declined(AutomatedRepairDeclineReason::HumanReviewAlreadyOpen);
        }
        if self.repair_attempt_limit_reached {
            return declined(AutomatedRepairDeclineReason::AttemptLimitReached);
        }
        if !self.opted_in_runner_available {
            return AutomatedRepairLoopStatus::WaitingForOptedInRunner;
        }
        if !self.runner_scope_and_capability_match {
            return declined(AutomatedRepairDeclineReason::RunnerScopeOrCapabilityMismatch);
        }
        if self.curator_or_admin_disabled {
            return declined(AutomatedRepairDeclineReason::CuratorOrAdminDisabled);
        }

        AutomatedRepairLoopStatus::Running
    }
}

fn declined(reason: AutomatedRepairDeclineReason) -> AutomatedRepairLoopStatus {
    AutomatedRepairLoopStatus::DeclinedByPolicy { reason }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicy {
    SandboxedCodeCritiquePythonChecker,
    SandboxedCodeRepairPythonChecker,
}

impl ExecutionPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionPolicy::SandboxedCodeCritiquePythonChecker => {
                "sandboxed_code_critique_python_checker"
            }
            ExecutionPolicy::SandboxedCodeRepairPythonChecker => {
                "sandboxed_code_repair_python_checker"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairAttemptState {
    Open,
    Claimed,
    Interrupted,
    Continued,
    ArtifactCreated,
    Stopped,
    Cancelled,
}

impl RepairAttemptState {
    fn can_claim(self) -> bool {
        matches!(
            self,
            RepairAttemptState::Open | RepairAttemptState::Continued
        )
    }

    fn can_reopen(self) -> bool {
        self == RepairAttemptState::Claimed
    }

    fn can_interrupt(self) -> bool {
        self == RepairAttemptState::Claimed
    }

    fn can_continue(self) -> bool {
        self == RepairAttemptState::Interrupted
    }

    fn can_create_artifact(self) -> bool {
        matches!(
            self,
            RepairAttemptState::Claimed | RepairAttemptState::Continued
        )
    }

    fn can_stop(self) -> bool {
        matches!(
            self,
            RepairAttemptState::Interrupted | RepairAttemptState::Continued
        )
    }

    fn is_terminal(self) -> bool {
        matches!(
            self,
            RepairAttemptState::ArtifactCreated
                | RepairAttemptState::Stopped
                | RepairAttemptState::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairAttempt {
    pub state: RepairAttemptState,
    pub repair_continuation_index: u32,
}

impl RepairAttempt {
    pub fn claim(self) -> Result<Self, CodeRepairPolicyError> {
        if !self.state.can_claim() {
            return Err(CodeRepairPolicyError::InvalidRepairAttemptTransition {
                from: self.state,
                action: RepairAttemptAction::Claim,
            });
        }

        Ok(Self {
            state: RepairAttemptState::Claimed,
            repair_continuation_index: self.repair_continuation_index,
        })
    }

    pub fn reopen_after_lease_end(self) -> Result<Self, CodeRepairPolicyError> {
        if !self.state.can_reopen() {
            return Err(CodeRepairPolicyError::InvalidRepairAttemptTransition {
                from: self.state,
                action: RepairAttemptAction::ReopenAfterLeaseEnd,
            });
        }

        Ok(Self {
            state: RepairAttemptState::Open,
            repair_continuation_index: self.repair_continuation_index,
        })
    }

    pub fn interrupt(self) -> Result<Self, CodeRepairPolicyError> {
        if !self.state.can_interrupt() {
            return Err(CodeRepairPolicyError::InvalidRepairAttemptTransition {
                from: self.state,
                action: RepairAttemptAction::Interrupt,
            });
        }

        Ok(Self {
            state: RepairAttemptState::Interrupted,
            repair_continuation_index: self.repair_continuation_index,
        })
    }

    pub fn continue_after_interruption(self) -> Result<Self, CodeRepairPolicyError> {
        if !self.state.can_continue() {
            return Err(CodeRepairPolicyError::InvalidRepairAttemptTransition {
                from: self.state,
                action: RepairAttemptAction::ContinueAfterInterruption,
            });
        }

        Ok(Self {
            state: RepairAttemptState::Continued,
            repair_continuation_index: self
                .repair_continuation_index
                .checked_add(1)
                .ok_or(CodeRepairPolicyError::ContinuationIndexOverflow)?,
        })
    }

    pub fn mark_artifact_created(self) -> Result<Self, CodeRepairPolicyError> {
        if !self.state.can_create_artifact() {
            return Err(CodeRepairPolicyError::InvalidRepairAttemptTransition {
                from: self.state,
                action: RepairAttemptAction::MarkArtifactCreated,
            });
        }

        Ok(Self {
            state: RepairAttemptState::ArtifactCreated,
            repair_continuation_index: self.repair_continuation_index,
        })
    }

    pub fn stop(self) -> Result<Self, CodeRepairPolicyError> {
        if !self.state.can_stop() {
            return Err(CodeRepairPolicyError::InvalidRepairAttemptTransition {
                from: self.state,
                action: RepairAttemptAction::Stop,
            });
        }

        Ok(Self {
            state: RepairAttemptState::Stopped,
            repair_continuation_index: self.repair_continuation_index,
        })
    }

    pub fn cancel(self) -> Result<Self, CodeRepairPolicyError> {
        if self.state.is_terminal() {
            return Err(CodeRepairPolicyError::InvalidRepairAttemptTransition {
                from: self.state,
                action: RepairAttemptAction::Cancel,
            });
        }

        Ok(Self {
            state: RepairAttemptState::Cancelled,
            repair_continuation_index: self.repair_continuation_index,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairAttemptAction {
    Claim,
    ReopenAfterLeaseEnd,
    Interrupt,
    ContinueAfterInterruption,
    MarkArtifactCreated,
    Stop,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairInterruptionReason {
    RunnerQuotaExhausted,
    RunnerOperatorBudgetExhausted,
    ProviderUnavailableLocal,
    SandboxUnavailable,
    SuspectedLoopingBug,
    SuspectedMaliciousTask,
    LocalPolicyRefused,
}

impl RepairInterruptionReason {
    fn routes_to_operator_review(self) -> bool {
        matches!(
            self,
            RepairInterruptionReason::RunnerQuotaExhausted
                | RepairInterruptionReason::RunnerOperatorBudgetExhausted
                | RepairInterruptionReason::ProviderUnavailableLocal
                | RepairInterruptionReason::SandboxUnavailable
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
#[serde(rename_all = "snake_case")]
pub enum SafeSummaryCode {
    LocalQuotaExhaustedBeforeOutput,
    OperatorBudgetExhaustedBeforeOutput,
    ProviderUnavailableBeforeOutput,
    SandboxUnavailableBeforeOutput,
    LoopSuspectedNoOutput,
    MaliciousTaskSuspectedNoOutput,
    LocalPolicyRefusedNoOutput,
}

impl SafeSummaryCode {
    pub fn matches_reason(self, reason: RepairInterruptionReason) -> bool {
        matches!(
            (self, reason),
            (
                SafeSummaryCode::LocalQuotaExhaustedBeforeOutput,
                RepairInterruptionReason::RunnerQuotaExhausted
            ) | (
                SafeSummaryCode::OperatorBudgetExhaustedBeforeOutput,
                RepairInterruptionReason::RunnerOperatorBudgetExhausted
            ) | (
                SafeSummaryCode::ProviderUnavailableBeforeOutput,
                RepairInterruptionReason::ProviderUnavailableLocal
            ) | (
                SafeSummaryCode::SandboxUnavailableBeforeOutput,
                RepairInterruptionReason::SandboxUnavailable
            ) | (
                SafeSummaryCode::LoopSuspectedNoOutput,
                RepairInterruptionReason::SuspectedLoopingBug
            ) | (
                SafeSummaryCode::MaliciousTaskSuspectedNoOutput,
                RepairInterruptionReason::SuspectedMaliciousTask
            ) | (
                SafeSummaryCode::LocalPolicyRefusedNoOutput,
                RepairInterruptionReason::LocalPolicyRefused
            )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationOutcome {
    ContinueOnAnotherRunner,
    StopExhaustedAttempts,
    StopPolicyRefused,
    QuarantineRequestOrArtifact,
    MarkRepairBugForHumanTriage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairContinuationSafeReason {
    QuotaExhaustedContinueElsewhere,
    OperatorBudgetExhaustedContinueElsewhere,
    ProviderUnavailableContinueElsewhere,
    SandboxUnavailableContinueElsewhere,
    LoopSuspectedStop,
    MaliciousTaskSuspectedStop,
    OperatorBudgetStop,
    CuratorQuarantine,
    HumanTriageRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairContinuationDecision {
    pub repair_continuation_decision_id: String,
    pub interruption_report_id: String,
    pub repair_attempt_id: String,
    pub source_artifact_id: String,
    pub repair_attempt_index: u32,
    pub actor_id: String,
    pub outcome: ContinuationOutcome,
    pub safe_reason_code: RepairContinuationSafeReason,
    pub interrupted_repair_continuation_index: u32,
    pub next_repair_continuation_index: Option<u32>,
    pub idempotency_key: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationIdempotencyReplay {
    NewCommand,
    ReplayedIdentical,
    ReplayedChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationContext {
    pub interruption_report_id: String,
    pub repair_attempt_id: String,
    pub source_artifact_id: String,
    pub repair_attempt_index: u32,
    pub interruption_reason: RepairInterruptionReason,
    pub repair_attempt_state: RepairAttemptState,
    pub automated_repair_status: AutomatedRepairLoopStatus,
    pub interrupted_repair_continuation_index: u32,
    pub repaired_artifact_exists: bool,
    pub continuation_count: u8,
    pub source_artifact_state: ArtifactState,
    pub actor_id: String,
    pub actor_type: ActorType,
    pub actor_scope_matches: bool,
    pub runner_operator_owns_interrupted_runner: bool,
    pub target_runner_is_eligible_and_opted_in: bool,
    pub target_runner_differs_from_interrupted_runner: bool,
    pub idempotency_replay: ContinuationIdempotencyReplay,
}

impl ContinuationContext {
    pub fn validate_decision(
        &self,
        decision: &RepairContinuationDecision,
    ) -> Result<(), CodeRepairPolicyError> {
        if decision.idempotency_key.is_empty()
            || !decision
                .idempotency_key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(CodeRepairPolicyError::InvalidContinuationIdempotencyKey);
        }
        if self.idempotency_replay == ContinuationIdempotencyReplay::ReplayedChanged {
            return Err(CodeRepairPolicyError::InvalidContinuationIdempotencyReplay);
        }
        require_context_match(
            decision.interruption_report_id == self.interruption_report_id,
            "interruption_report_id",
        )?;
        require_context_match(
            decision.repair_attempt_id == self.repair_attempt_id,
            "repair_attempt_id",
        )?;
        require_context_match(
            decision.source_artifact_id == self.source_artifact_id,
            "source_artifact_id",
        )?;
        require_context_match(
            decision.repair_attempt_index == self.repair_attempt_index,
            "repair_attempt_index",
        )?;
        require_context_match(decision.actor_id == self.actor_id, "actor_id")?;
        if !self.actor_type.can_issue_repair_continuation() {
            return Err(CodeRepairPolicyError::UnauthorizedContinuationActor {
                actor_type: self.actor_type,
            });
        }
        if !self.actor_scope_matches {
            return Err(CodeRepairPolicyError::ContinuationScopeMismatch);
        }
        if self.actor_type == ActorType::RunnerOperator
            && !self.runner_operator_owns_interrupted_runner
        {
            return Err(CodeRepairPolicyError::RunnerOperatorDoesNotOwnInterruptedRunner);
        }
        if self.repair_attempt_state != RepairAttemptState::Interrupted {
            return Err(CodeRepairPolicyError::RepairAttemptMustBeInterrupted {
                state: self.repair_attempt_state,
            });
        }
        if !matches!(
            self.automated_repair_status,
            AutomatedRepairLoopStatus::WaitingForOperatorReview
                | AutomatedRepairLoopStatus::NeedsOperatorReview
        ) {
            return Err(CodeRepairPolicyError::InvalidAutomatedRepairStatus {
                status: self.automated_repair_status,
            });
        }
        if self.repaired_artifact_exists {
            return Err(CodeRepairPolicyError::RepairedArtifactAlreadyExists);
        }
        if self.continuation_count >= 2 {
            return Err(CodeRepairPolicyError::ContinuationLimitReached);
        }
        if self.source_artifact_state.blocks_repair_continuation() {
            return Err(
                CodeRepairPolicyError::SourceArtifactStateBlocksContinuation {
                    state: self.source_artifact_state,
                },
            );
        }
        if decision.interrupted_repair_continuation_index
            != self.interrupted_repair_continuation_index
        {
            return Err(
                CodeRepairPolicyError::InterruptedContinuationIndexMismatch {
                    expected: self.interrupted_repair_continuation_index,
                    actual: decision.interrupted_repair_continuation_index,
                },
            );
        }

        self.validate_index_shape(decision)?;
        self.validate_reason_mapping(decision)
    }

    fn validate_index_shape(
        &self,
        decision: &RepairContinuationDecision,
    ) -> Result<(), CodeRepairPolicyError> {
        if decision.outcome == ContinuationOutcome::ContinueOnAnotherRunner {
            let expected_next = self
                .interrupted_repair_continuation_index
                .checked_add(1)
                .ok_or(CodeRepairPolicyError::ContinuationIndexOverflow)?;
            if decision.next_repair_continuation_index != Some(expected_next) {
                return Err(CodeRepairPolicyError::InvalidNextContinuationIndex {
                    expected: expected_next,
                    actual: decision.next_repair_continuation_index,
                });
            }
            if !self.target_runner_is_eligible_and_opted_in {
                return Err(CodeRepairPolicyError::ContinuationTargetRunnerNotEligible);
            }
            if !self.target_runner_differs_from_interrupted_runner {
                return Err(CodeRepairPolicyError::ContinuationTargetRunnerMustDiffer);
            }
            return Ok(());
        }

        if decision.next_repair_continuation_index.is_some() {
            return Err(CodeRepairPolicyError::UnexpectedNextContinuationIndex);
        }

        Ok(())
    }

    fn validate_reason_mapping(
        &self,
        decision: &RepairContinuationDecision,
    ) -> Result<(), CodeRepairPolicyError> {
        let valid = match self.interruption_reason {
            RepairInterruptionReason::RunnerQuotaExhausted => matches!(
                (decision.outcome, decision.safe_reason_code),
                (
                    ContinuationOutcome::ContinueOnAnotherRunner,
                    RepairContinuationSafeReason::QuotaExhaustedContinueElsewhere
                ) | (
                    ContinuationOutcome::StopExhaustedAttempts,
                    RepairContinuationSafeReason::OperatorBudgetStop
                ) | (
                    ContinuationOutcome::MarkRepairBugForHumanTriage,
                    RepairContinuationSafeReason::HumanTriageRequired
                )
            ),
            RepairInterruptionReason::RunnerOperatorBudgetExhausted => matches!(
                (decision.outcome, decision.safe_reason_code),
                (
                    ContinuationOutcome::ContinueOnAnotherRunner,
                    RepairContinuationSafeReason::OperatorBudgetExhaustedContinueElsewhere
                ) | (
                    ContinuationOutcome::StopExhaustedAttempts,
                    RepairContinuationSafeReason::OperatorBudgetStop
                ) | (
                    ContinuationOutcome::MarkRepairBugForHumanTriage,
                    RepairContinuationSafeReason::HumanTriageRequired
                )
            ),
            RepairInterruptionReason::ProviderUnavailableLocal => matches!(
                (decision.outcome, decision.safe_reason_code),
                (
                    ContinuationOutcome::ContinueOnAnotherRunner,
                    RepairContinuationSafeReason::ProviderUnavailableContinueElsewhere
                ) | (
                    ContinuationOutcome::StopExhaustedAttempts,
                    RepairContinuationSafeReason::OperatorBudgetStop
                ) | (
                    ContinuationOutcome::MarkRepairBugForHumanTriage,
                    RepairContinuationSafeReason::HumanTriageRequired
                )
            ),
            RepairInterruptionReason::SandboxUnavailable => matches!(
                (decision.outcome, decision.safe_reason_code),
                (
                    ContinuationOutcome::ContinueOnAnotherRunner,
                    RepairContinuationSafeReason::SandboxUnavailableContinueElsewhere
                ) | (
                    ContinuationOutcome::StopExhaustedAttempts,
                    RepairContinuationSafeReason::OperatorBudgetStop
                ) | (
                    ContinuationOutcome::MarkRepairBugForHumanTriage,
                    RepairContinuationSafeReason::HumanTriageRequired
                )
            ),
            RepairInterruptionReason::SuspectedLoopingBug => matches!(
                (decision.outcome, decision.safe_reason_code),
                (
                    ContinuationOutcome::StopExhaustedAttempts,
                    RepairContinuationSafeReason::LoopSuspectedStop
                ) | (
                    ContinuationOutcome::MarkRepairBugForHumanTriage,
                    RepairContinuationSafeReason::HumanTriageRequired
                )
            ),
            RepairInterruptionReason::SuspectedMaliciousTask => matches!(
                (decision.outcome, decision.safe_reason_code),
                (
                    ContinuationOutcome::QuarantineRequestOrArtifact,
                    RepairContinuationSafeReason::CuratorQuarantine
                ) | (
                    ContinuationOutcome::MarkRepairBugForHumanTriage,
                    RepairContinuationSafeReason::MaliciousTaskSuspectedStop
                )
            ),
            RepairInterruptionReason::LocalPolicyRefused => matches!(
                (decision.outcome, decision.safe_reason_code),
                (
                    ContinuationOutcome::StopPolicyRefused,
                    RepairContinuationSafeReason::OperatorBudgetStop
                ) | (
                    ContinuationOutcome::MarkRepairBugForHumanTriage,
                    RepairContinuationSafeReason::HumanTriageRequired
                )
            ),
        };

        if !valid {
            return Err(
                CodeRepairPolicyError::InvalidContinuationDecisionForInterruption {
                    interruption_reason: self.interruption_reason,
                    outcome: decision.outcome,
                    safe_reason_code: decision.safe_reason_code,
                },
            );
        }

        if decision.outcome == ContinuationOutcome::ContinueOnAnotherRunner
            && !self.interruption_reason.routes_to_operator_review()
        {
            return Err(
                CodeRepairPolicyError::ContinuationNotAllowedForInterruption {
                    interruption_reason: self.interruption_reason,
                },
            );
        }

        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodeRepairPolicyError {
    #[error("actor type {actor_type:?} cannot issue repair continuation decisions")]
    UnauthorizedContinuationActor { actor_type: ActorType },
    #[error("repair attempt must be interrupted before continuation decisions, got {state:?}")]
    RepairAttemptMustBeInterrupted { state: RepairAttemptState },
    #[error("automated repair status {status:?} cannot accept a continuation decision")]
    InvalidAutomatedRepairStatus { status: AutomatedRepairLoopStatus },
    #[error("repair attempt already produced an artifact")]
    RepairedArtifactAlreadyExists,
    #[error("repair continuation limit reached")]
    ContinuationLimitReached,
    #[error("source artifact state {state:?} blocks repair continuation")]
    SourceArtifactStateBlocksContinuation { state: ArtifactState },
    #[error("interrupted continuation index mismatch: expected {expected}, got {actual}")]
    InterruptedContinuationIndexMismatch { expected: u32, actual: u32 },
    #[error("invalid next continuation index: expected {expected}, got {actual:?}")]
    InvalidNextContinuationIndex { expected: u32, actual: Option<u32> },
    #[error("next continuation index is only allowed for continue_on_another_runner")]
    UnexpectedNextContinuationIndex,
    #[error(
        "invalid continuation decision {outcome:?}/{safe_reason_code:?} for interruption {interruption_reason:?}"
    )]
    InvalidContinuationDecisionForInterruption {
        interruption_reason: RepairInterruptionReason,
        outcome: ContinuationOutcome,
        safe_reason_code: RepairContinuationSafeReason,
    },
    #[error("continuation is not allowed for interruption {interruption_reason:?}")]
    ContinuationNotAllowedForInterruption {
        interruption_reason: RepairInterruptionReason,
    },
    #[error("invalid repair attempt transition from {from:?} via {action:?}")]
    InvalidRepairAttemptTransition {
        from: RepairAttemptState,
        action: RepairAttemptAction,
    },
    #[error("invalid work packet transition from {from:?} via {action:?}")]
    InvalidWorkPacketTransition {
        from: WorkPacketState,
        action: WorkPacketTransition,
    },
    #[error("repair continuation index overflow")]
    ContinuationIndexOverflow,
    #[error("continuation actor scope does not match interrupted repair")]
    ContinuationScopeMismatch,
    #[error("runner operator does not own interrupted runner")]
    RunnerOperatorDoesNotOwnInterruptedRunner,
    #[error("continuation target runner is not eligible and opted in")]
    ContinuationTargetRunnerNotEligible,
    #[error("continuation target runner must differ from interrupted runner")]
    ContinuationTargetRunnerMustDiffer,
    #[error("invalid continuation idempotency replay")]
    InvalidContinuationIdempotencyReplay,
    #[error("invalid continuation idempotency key")]
    InvalidContinuationIdempotencyKey,
    #[error("invalid execution policy: expected {expected:?}, got {actual:?}")]
    InvalidExecutionPolicy {
        expected: ExecutionPolicy,
        actual: ExecutionPolicy,
    },
    #[error(
        "safe summary code {safe_summary_code:?} does not match interruption reason {interruption_reason:?}"
    )]
    SafeSummaryCodeMismatch {
        interruption_reason: RepairInterruptionReason,
        safe_summary_code: SafeSummaryCode,
    },
    #[error("invalid reviewed files for MVP code critique")]
    InvalidReviewedFiles,
    #[error("invalid check result for {check:?}")]
    InvalidCheckResult { check: CodeCheckName },
    #[error("required check coverage mismatch")]
    RequiredCheckCoverageMismatch,
    #[error("invalid repair report changed files")]
    InvalidChangedFiles,
    #[error("invalid digest string in {field}")]
    InvalidDigest { field: &'static str },
    #[error("non-code repaired file digest differs from source digest")]
    NonCodeDigestChanged,
    #[error("invalid code repair execution profile or command")]
    InvalidRepairExecutionBinding,
    #[error("invalid code critique execution profile or command")]
    InvalidCritiqueExecutionBinding,
    #[error("invalid repair sandbox status for repair status")]
    InvalidRepairSandboxStatus,
    #[error("invalid attestation")]
    InvalidAttestation,
    #[error("attestation key is inactive")]
    InactiveAttestationKey,
    #[error("attestation key does not match report context")]
    AttestationKeyMismatch,
    #[error("attestation signature verification failed")]
    AttestationSignatureInvalid,
    #[error("server-derived report context mismatch in {field}")]
    ReportContextMismatch { field: &'static str },
    #[error("could not build unsigned attestation payload")]
    UnsignedPayloadBuildFailed,
    #[error("unsafe submitted text in {field}")]
    UnsafeSubmittedText { field: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attestation {
    pub attestation_schema_version: String,
    pub signature_kind: String,
    pub runner_key_id: String,
    pub signed_payload_digest: String,
    pub signature: String,
}

impl Attestation {
    pub fn validate(&self) -> Result<(), CodeRepairPolicyError> {
        if self.attestation_schema_version != "runner-self-test-attestation-v1"
            || self.signature_kind != "ed25519"
            || !self.runner_key_id.starts_with("rkey_")
            || !is_sha256_digest(&self.signed_payload_digest)
            || self.signature.is_empty()
            || !self
                .signature
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(CodeRepairPolicyError::InvalidAttestation);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredRunnerKey {
    pub runner_actor_id: String,
    pub runner_key_id: String,
    pub public_key_bytes: [u8; 32],
    pub is_active: bool,
}

impl RegisteredRunnerKey {
    fn verify_report_attestation<T: Serialize>(
        &self,
        report: &T,
        report_runner_actor_id: &str,
        attestation: &Attestation,
    ) -> Result<(), CodeRepairPolicyError> {
        attestation.validate()?;
        validate_attested_payload_digest(report, attestation)?;

        if !self.is_active {
            return Err(CodeRepairPolicyError::InactiveAttestationKey);
        }
        if self.runner_actor_id != report_runner_actor_id
            || self.runner_key_id != attestation.runner_key_id
        {
            return Err(CodeRepairPolicyError::AttestationKeyMismatch);
        }

        let verifying_key = VerifyingKey::from_bytes(&self.public_key_bytes)
            .map_err(|_| CodeRepairPolicyError::InvalidAttestation)?;
        let signature_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&attestation.signature)
            .map_err(|_| CodeRepairPolicyError::InvalidAttestation)?;
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|_| CodeRepairPolicyError::InvalidAttestation)?;
        let payload = unsigned_payload_canonical_bytes(report)?;
        verifying_key
            .verify(&payload, &signature)
            .map_err(|_| CodeRepairPolicyError::AttestationSignatureInvalid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeRepairInterruptionReport {
    pub interruption_report_id: String,
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub source_artifact_id: String,
    pub source_bundle_digest: String,
    pub repair_attempt_id: String,
    pub repair_attempt_index: u32,
    pub repair_continuation_index: u32,
    pub repair_root_artifact_id: String,
    pub repair_parent_artifact_id: String,
    pub execution_policy: ExecutionPolicy,
    pub interruption_reason: RepairInterruptionReason,
    pub safe_summary_code: SafeSummaryCode,
    pub attestation: Attestation,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_critique_report_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_work_digest: Option<String>,
}

impl CodeRepairInterruptionReport {
    fn validate_structure(&self) -> Result<(), CodeRepairPolicyError> {
        self.attestation.validate()?;
        validate_attested_payload_digest(self, &self.attestation)?;
        if !is_sha256_digest(&self.source_bundle_digest) {
            return Err(CodeRepairPolicyError::InvalidDigest {
                field: "source_bundle_digest",
            });
        }
        if self
            .partial_work_digest
            .as_ref()
            .is_some_and(|digest| !is_sha256_digest(digest))
        {
            return Err(CodeRepairPolicyError::InvalidDigest {
                field: "partial_work_digest",
            });
        }
        if self.execution_policy != ExecutionPolicy::SandboxedCodeRepairPythonChecker {
            return Err(CodeRepairPolicyError::InvalidExecutionPolicy {
                expected: ExecutionPolicy::SandboxedCodeRepairPythonChecker,
                actual: self.execution_policy,
            });
        }
        if !self
            .safe_summary_code
            .matches_reason(self.interruption_reason)
        {
            return Err(CodeRepairPolicyError::SafeSummaryCodeMismatch {
                interruption_reason: self.interruption_reason,
                safe_summary_code: self.safe_summary_code,
            });
        }

        Ok(())
    }

    pub fn unsigned_payload_value(&self) -> Result<serde_json::Value, CodeRepairPolicyError> {
        unsigned_payload_value(self)
    }

    pub fn refresh_attestation_payload_digest(&mut self) -> Result<(), CodeRepairPolicyError> {
        self.attestation.signed_payload_digest = unsigned_payload_digest(self)?;
        Ok(())
    }

    pub fn validate_against_context(
        &self,
        context: &CodeRepairInterruptionReportContext,
    ) -> Result<(), CodeRepairPolicyError> {
        self.validate_structure()?;
        context.registered_runner_key.verify_report_attestation(
            self,
            &self.runner_actor_id,
            &self.attestation,
        )?;
        require_context_match(
            self.work_packet_id == context.work_packet_id,
            "work_packet_id",
        )?;
        require_context_match(self.lease_id == context.lease_id, "lease_id")?;
        require_context_match(
            self.runner_actor_id == context.runner_actor_id,
            "runner_actor_id",
        )?;
        require_context_match(
            self.source_artifact_id == context.source_artifact_id,
            "source_artifact_id",
        )?;
        require_context_match(
            self.source_bundle_digest == context.source_bundle_digest,
            "source_bundle_digest",
        )?;
        require_context_match(
            self.repair_attempt_id == context.repair_attempt_id,
            "repair_attempt_id",
        )?;
        require_context_match(
            self.repair_attempt_index == context.repair_attempt_index,
            "repair_attempt_index",
        )?;
        require_context_match(
            self.repair_continuation_index == context.repair_continuation_index,
            "repair_continuation_index",
        )?;
        require_context_match(
            self.repair_root_artifact_id == context.repair_root_artifact_id,
            "repair_root_artifact_id",
        )?;
        require_context_match(
            self.repair_parent_artifact_id == context.repair_parent_artifact_id,
            "repair_parent_artifact_id",
        )?;
        require_context_match(
            self.source_critique_report_id == context.source_critique_report_id,
            "source_critique_report_id",
        )?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRepairInterruptionReportContext {
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub source_artifact_id: String,
    pub source_bundle_digest: String,
    pub repair_attempt_id: String,
    pub repair_attempt_index: u32,
    pub repair_continuation_index: u32,
    pub repair_root_artifact_id: String,
    pub repair_parent_artifact_id: String,
    pub source_critique_report_id: Option<String>,
    pub registered_runner_key: RegisteredRunnerKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeRepairStatus {
    RepairProposed,
    RepairProposedStaticOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairSandboxStatus {
    Enforced,
    NotRunStaticOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeRepairReport {
    pub repair_report_id: String,
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub source_artifact_id: String,
    pub source_bundle_digest: String,
    pub repair_attempt_id: String,
    pub repair_attempt_index: u32,
    pub repair_continuation_index: u32,
    pub repair_root_artifact_id: String,
    pub repair_parent_artifact_id: String,
    pub target_artifact_id: String,
    pub target_artifact_intake_ref: String,
    pub execution_policy: ExecutionPolicy,
    pub execution_profile_id: String,
    pub allowed_command_id: String,
    pub sandbox_status: RepairSandboxStatus,
    pub repair_status: CodeRepairStatus,
    pub changed_files: Vec<String>,
    pub source_file_digests: FileDigestSet,
    pub repaired_file_digests: FileDigestSet,
    pub repaired_bundle_digest: String,
    pub checks: Vec<CodeCheckResult>,
    pub attestation: Attestation,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_critique_report_id: Option<String>,
}

impl CodeRepairReport {
    fn validate_structure(&self) -> Result<(), CodeRepairPolicyError> {
        self.attestation.validate()?;
        validate_attested_payload_digest(self, &self.attestation)?;
        if self.execution_policy != ExecutionPolicy::SandboxedCodeRepairPythonChecker {
            return Err(CodeRepairPolicyError::InvalidExecutionPolicy {
                expected: ExecutionPolicy::SandboxedCodeRepairPythonChecker,
                actual: self.execution_policy,
            });
        }
        if !is_sha256_digest(&self.source_bundle_digest) {
            return Err(CodeRepairPolicyError::InvalidDigest {
                field: "source_bundle_digest",
            });
        }
        if self.execution_profile_id != "python_checker_code_repair_v1"
            || self.allowed_command_id != "python_checker_code_repair_harness_v1"
        {
            return Err(CodeRepairPolicyError::InvalidRepairExecutionBinding);
        }
        if self.repair_status == CodeRepairStatus::RepairProposed
            && self.sandbox_status != RepairSandboxStatus::Enforced
        {
            return Err(CodeRepairPolicyError::InvalidRepairSandboxStatus);
        }
        if self.repair_status == CodeRepairStatus::RepairProposedStaticOnly
            && self.sandbox_status != RepairSandboxStatus::NotRunStaticOnly
        {
            return Err(CodeRepairPolicyError::InvalidRepairSandboxStatus);
        }
        if self.changed_files != ["checker.py"] {
            return Err(CodeRepairPolicyError::InvalidChangedFiles);
        }
        self.source_file_digests.validate("source_file_digests")?;
        self.repaired_file_digests
            .validate("repaired_file_digests")?;
        if !is_sha256_digest(&self.repaired_bundle_digest) {
            return Err(CodeRepairPolicyError::InvalidDigest {
                field: "repaired_bundle_digest",
            });
        }
        if !self
            .source_file_digests
            .non_code_digests_match(&self.repaired_file_digests)
        {
            return Err(CodeRepairPolicyError::NonCodeDigestChanged);
        }
        validate_required_checks(&self.checks, required_repair_checks())?;

        Ok(())
    }

    pub fn validate_against_context(
        &self,
        context: &CodeRepairReportContext,
    ) -> Result<(), CodeRepairPolicyError> {
        self.validate_structure()?;
        context.registered_runner_key.verify_report_attestation(
            self,
            &self.runner_actor_id,
            &self.attestation,
        )?;
        require_context_match(
            self.work_packet_id == context.work_packet_id,
            "work_packet_id",
        )?;
        require_context_match(self.lease_id == context.lease_id, "lease_id")?;
        require_context_match(
            self.runner_actor_id == context.runner_actor_id,
            "runner_actor_id",
        )?;
        require_context_match(
            self.source_artifact_id == context.source_artifact_id,
            "source_artifact_id",
        )?;
        require_context_match(
            self.source_bundle_digest == context.source_bundle_digest,
            "source_bundle_digest",
        )?;
        require_context_match(
            self.repair_attempt_id == context.repair_attempt_id,
            "repair_attempt_id",
        )?;
        require_context_match(
            self.repair_attempt_index == context.repair_attempt_index,
            "repair_attempt_index",
        )?;
        require_context_match(
            self.repair_continuation_index == context.repair_continuation_index,
            "repair_continuation_index",
        )?;
        require_context_match(
            self.repair_root_artifact_id == context.repair_root_artifact_id,
            "repair_root_artifact_id",
        )?;
        require_context_match(
            self.repair_parent_artifact_id == context.repair_parent_artifact_id,
            "repair_parent_artifact_id",
        )?;
        require_context_match(
            self.target_artifact_id == context.target_artifact_id,
            "target_artifact_id",
        )?;
        require_context_match(
            self.target_artifact_intake_ref == context.target_artifact_intake_ref,
            "target_artifact_intake_ref",
        )?;
        require_context_match(
            self.source_critique_report_id == context.source_critique_report_id,
            "source_critique_report_id",
        )?;
        require_context_match(
            self.source_file_digests == context.authoritative_source_file_digests,
            "source_file_digests",
        )?;
        require_context_match(
            self.repaired_file_digests == context.authoritative_repaired_file_digests,
            "repaired_file_digests",
        )?;
        require_context_match(
            self.repaired_bundle_digest == context.authoritative_repaired_bundle_digest,
            "repaired_bundle_digest",
        )?;

        Ok(())
    }

    pub fn unsigned_payload_value(&self) -> Result<serde_json::Value, CodeRepairPolicyError> {
        unsigned_payload_value(self)
    }

    pub fn refresh_attestation_payload_digest(&mut self) -> Result<(), CodeRepairPolicyError> {
        self.attestation.signed_payload_digest = unsigned_payload_digest(self)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRepairReportContext {
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub source_artifact_id: String,
    pub source_bundle_digest: String,
    pub repair_attempt_id: String,
    pub repair_attempt_index: u32,
    pub repair_continuation_index: u32,
    pub repair_root_artifact_id: String,
    pub repair_parent_artifact_id: String,
    pub target_artifact_id: String,
    pub target_artifact_intake_ref: String,
    pub source_critique_report_id: Option<String>,
    pub authoritative_source_file_digests: FileDigestSet,
    pub authoritative_repaired_file_digests: FileDigestSet,
    pub authoritative_repaired_bundle_digest: String,
    pub registered_runner_key: RegisteredRunnerKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileDigestSet {
    #[serde(rename = "manifest.json")]
    pub manifest_json: String,
    #[serde(rename = "worksheet.md")]
    pub worksheet_md: String,
    #[serde(rename = "answer_key.md")]
    pub answer_key_md: String,
    #[serde(rename = "teacher_notes.md")]
    pub teacher_notes_md: String,
    #[serde(rename = "checker.py")]
    pub checker_py: String,
}

impl FileDigestSet {
    fn validate(&self, field: &'static str) -> Result<(), CodeRepairPolicyError> {
        if [
            &self.manifest_json,
            &self.worksheet_md,
            &self.answer_key_md,
            &self.teacher_notes_md,
            &self.checker_py,
        ]
        .iter()
        .all(|digest| is_sha256_digest(digest))
        {
            return Ok(());
        }

        Err(CodeRepairPolicyError::InvalidDigest { field })
    }

    fn non_code_digests_match(&self, repaired: &Self) -> bool {
        self.manifest_json == repaired.manifest_json
            && self.worksheet_md == repaired.worksheet_md
            && self.answer_key_md == repaired.answer_key_md
            && self.teacher_notes_md == repaired.teacher_notes_md
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    Enforced,
    NotRunPolicyDisabled,
    NotRunSandboxUnavailable,
    NotRunRunnerConfigDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeCritiqueOutcome {
    NoCodeFindings,
    BlockingCodeFindings,
    NonblockingCodeFindings,
    RepairRecommended,
    InvalidInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingType {
    UnsafeImport,
    FilesystemAccess,
    NetworkAccess,
    SubprocessExecution,
    DynamicCodeExecution,
    ReflectionOrIntrospection,
    Concurrency,
    ContractMismatch,
    SampleCaseFailure,
    SelfReportedSuccess,
    Nondeterminism,
    StyleOrMaintainability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Critical,
    Major,
    Minor,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    StaticAst,
    SandboxSampleCase,
    SchemaCheck,
    DigestCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairHintCode {
    RemoveForbiddenImport,
    ReplaceIoWithPureFunction,
    MatchExpectedFunctionContract,
    FixSampleCaseLogic,
    RemoveSelfReportedSuccess,
    MakeDeterministic,
    ManualReviewRecommended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeCheckName {
    SourceDigestVerified,
    SandboxProfileEnforced,
    CheckerStaticSafety,
    CheckerFunctionContract,
    CheckerSampleCases,
    NoNetworkObserved,
    NoFilesystemEscapeObserved,
    NoSecretEnvPresent,
    RawOutputRedacted,
    TargetIdsMatchWorkPacket,
    ChangedFilesLimited,
    NonCodeFilesPreserved,
    RepairedDigestComputed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSafeReasonCode {
    SourceDigestMismatch,
    SandboxUnavailable,
    StaticSafetyFailed,
    FunctionContractMismatch,
    SampleCasesFailed,
    NetworkObserved,
    FilesystemEscapeObserved,
    SecretEnvPresent,
    RawOutputRedactedUnavailable,
    TargetIdsMismatch,
    ChangedFilesNotLimited,
    NonCodeFilesChanged,
    RepairedDigestNotComputed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeCheckResult {
    pub check: CodeCheckName,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_reason_code: Option<CheckSafeReasonCode>,
}

impl CodeCheckResult {
    pub fn passed(check: CodeCheckName) -> Self {
        Self {
            check,
            status: CheckStatus::Passed,
            safe_reason_code: None,
        }
    }

    pub fn validate(&self) -> Result<(), CodeRepairPolicyError> {
        match (self.status, self.safe_reason_code) {
            (CheckStatus::Passed, None) => Ok(()),
            (CheckStatus::Passed, Some(_)) | (CheckStatus::Failed | CheckStatus::NotRun, None) => {
                Err(CodeRepairPolicyError::InvalidCheckResult { check: self.check })
            }
            (CheckStatus::Failed | CheckStatus::NotRun, Some(reason))
                if self.reason_matches_check(reason) =>
            {
                Ok(())
            }
            (CheckStatus::Failed | CheckStatus::NotRun, Some(_)) => {
                Err(CodeRepairPolicyError::InvalidCheckResult { check: self.check })
            }
        }
    }

    fn reason_matches_check(&self, reason: CheckSafeReasonCode) -> bool {
        matches!(
            (self.check, reason),
            (
                CodeCheckName::SourceDigestVerified,
                CheckSafeReasonCode::SourceDigestMismatch
            ) | (
                CodeCheckName::SandboxProfileEnforced,
                CheckSafeReasonCode::SandboxUnavailable
            ) | (
                CodeCheckName::CheckerStaticSafety,
                CheckSafeReasonCode::StaticSafetyFailed
            ) | (
                CodeCheckName::CheckerFunctionContract,
                CheckSafeReasonCode::FunctionContractMismatch
            ) | (
                CodeCheckName::CheckerSampleCases,
                CheckSafeReasonCode::SampleCasesFailed
            ) | (
                CodeCheckName::NoNetworkObserved,
                CheckSafeReasonCode::NetworkObserved
            ) | (
                CodeCheckName::NoFilesystemEscapeObserved,
                CheckSafeReasonCode::FilesystemEscapeObserved
            ) | (
                CodeCheckName::NoSecretEnvPresent,
                CheckSafeReasonCode::SecretEnvPresent
            ) | (
                CodeCheckName::RawOutputRedacted,
                CheckSafeReasonCode::RawOutputRedactedUnavailable
            ) | (
                CodeCheckName::TargetIdsMatchWorkPacket,
                CheckSafeReasonCode::TargetIdsMismatch
            ) | (
                CodeCheckName::ChangedFilesLimited,
                CheckSafeReasonCode::ChangedFilesNotLimited
            ) | (
                CodeCheckName::NonCodeFilesPreserved,
                CheckSafeReasonCode::NonCodeFilesChanged
            ) | (
                CodeCheckName::RepairedDigestComputed,
                CheckSafeReasonCode::RepairedDigestNotComputed
            )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeCritiqueFinding {
    pub finding_type: FindingType,
    pub severity: FindingSeverity,
    pub safe_location: String,
    pub safe_message: String,
    pub evidence_kind: EvidenceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_hint_code: Option<RepairHintCode>,
}

impl CodeCritiqueFinding {
    pub fn validate(&self) -> Result<(), CodeRepairPolicyError> {
        validate_safe_location(&self.safe_location)?;
        validate_safe_message(&self.safe_message)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeCritiqueReport {
    pub critique_report_id: String,
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub source_artifact_id: String,
    pub source_bundle_digest: String,
    pub execution_policy: ExecutionPolicy,
    pub execution_profile_id: String,
    pub allowed_command_id: String,
    pub sandbox_status: SandboxStatus,
    pub outcome: CodeCritiqueOutcome,
    pub reviewed_files: Vec<String>,
    pub checks: Vec<CodeCheckResult>,
    pub findings: Vec<CodeCritiqueFinding>,
    pub attestation: Attestation,
    pub created_at: String,
}

impl CodeCritiqueReport {
    fn validate_structure(&self) -> Result<(), CodeRepairPolicyError> {
        self.attestation.validate()?;
        validate_attested_payload_digest(self, &self.attestation)?;
        if self.execution_policy != ExecutionPolicy::SandboxedCodeCritiquePythonChecker {
            return Err(CodeRepairPolicyError::InvalidExecutionPolicy {
                expected: ExecutionPolicy::SandboxedCodeCritiquePythonChecker,
                actual: self.execution_policy,
            });
        }
        if !is_sha256_digest(&self.source_bundle_digest) {
            return Err(CodeRepairPolicyError::InvalidDigest {
                field: "source_bundle_digest",
            });
        }
        if self.execution_profile_id != "python_checker_code_critique_v1"
            || self.allowed_command_id != "python_checker_code_critique_harness_v1"
        {
            return Err(CodeRepairPolicyError::InvalidCritiqueExecutionBinding);
        }
        if self.reviewed_files != ["checker.py"] {
            return Err(CodeRepairPolicyError::InvalidReviewedFiles);
        }
        validate_required_checks(&self.checks, required_critique_checks())?;

        for finding in &self.findings {
            finding.validate()?;
        }

        Ok(())
    }

    pub fn unsigned_payload_value(&self) -> Result<serde_json::Value, CodeRepairPolicyError> {
        unsigned_payload_value(self)
    }

    pub fn refresh_attestation_payload_digest(&mut self) -> Result<(), CodeRepairPolicyError> {
        self.attestation.signed_payload_digest = unsigned_payload_digest(self)?;
        Ok(())
    }

    pub fn validate_against_context(
        &self,
        context: &CodeCritiqueReportContext,
    ) -> Result<(), CodeRepairPolicyError> {
        self.validate_structure()?;
        context.registered_runner_key.verify_report_attestation(
            self,
            &self.runner_actor_id,
            &self.attestation,
        )?;
        require_context_match(
            self.work_packet_id == context.work_packet_id,
            "work_packet_id",
        )?;
        require_context_match(self.lease_id == context.lease_id, "lease_id")?;
        require_context_match(
            self.runner_actor_id == context.runner_actor_id,
            "runner_actor_id",
        )?;
        require_context_match(
            self.source_artifact_id == context.source_artifact_id,
            "source_artifact_id",
        )?;
        require_context_match(
            self.source_bundle_digest == context.source_bundle_digest,
            "source_bundle_digest",
        )?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeCritiqueReportContext {
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub source_artifact_id: String,
    pub source_bundle_digest: String,
    pub registered_runner_key: RegisteredRunnerKey,
}

fn require_context_match(
    matches_context: bool,
    field: &'static str,
) -> Result<(), CodeRepairPolicyError> {
    if matches_context {
        return Ok(());
    }

    Err(CodeRepairPolicyError::ReportContextMismatch { field })
}

fn unsigned_payload_value<T: Serialize>(
    report: &T,
) -> Result<serde_json::Value, CodeRepairPolicyError> {
    let mut value = serde_json::to_value(report)
        .map_err(|_| CodeRepairPolicyError::UnsignedPayloadBuildFailed)?;
    let serde_json::Value::Object(ref mut fields) = value else {
        return Err(CodeRepairPolicyError::UnsignedPayloadBuildFailed);
    };
    fields.remove("attestation");
    Ok(value)
}

fn validate_attested_payload_digest<T: Serialize>(
    report: &T,
    attestation: &Attestation,
) -> Result<(), CodeRepairPolicyError> {
    if unsigned_payload_digest(report)? == attestation.signed_payload_digest {
        return Ok(());
    }

    Err(CodeRepairPolicyError::InvalidAttestation)
}

pub fn unsigned_payload_digest<T: Serialize>(report: &T) -> Result<String, CodeRepairPolicyError> {
    let bytes = unsigned_payload_canonical_bytes(report)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{digest:x}"))
}

pub fn unsigned_payload_canonical_bytes<T: Serialize>(
    report: &T,
) -> Result<Vec<u8>, CodeRepairPolicyError> {
    let value = unsigned_payload_value(report)?;
    serde_jcs::to_vec(&value).map_err(|_| CodeRepairPolicyError::UnsignedPayloadBuildFailed)
}

fn validate_required_checks(
    checks: &[CodeCheckResult],
    required: &[CodeCheckName],
) -> Result<(), CodeRepairPolicyError> {
    let mut seen = BTreeSet::new();
    for check in checks {
        check.validate()?;
        if !required.contains(&check.check) || !seen.insert(check.check) {
            return Err(CodeRepairPolicyError::RequiredCheckCoverageMismatch);
        }
    }

    let required_set = required.iter().copied().collect::<BTreeSet<_>>();
    if seen != required_set {
        return Err(CodeRepairPolicyError::RequiredCheckCoverageMismatch);
    }

    Ok(())
}

fn required_critique_checks() -> &'static [CodeCheckName] {
    &[
        CodeCheckName::SourceDigestVerified,
        CodeCheckName::SandboxProfileEnforced,
        CodeCheckName::CheckerStaticSafety,
        CodeCheckName::CheckerFunctionContract,
        CodeCheckName::CheckerSampleCases,
        CodeCheckName::NoNetworkObserved,
        CodeCheckName::NoFilesystemEscapeObserved,
        CodeCheckName::NoSecretEnvPresent,
        CodeCheckName::RawOutputRedacted,
    ]
}

fn required_repair_checks() -> &'static [CodeCheckName] {
    &[
        CodeCheckName::SourceDigestVerified,
        CodeCheckName::TargetIdsMatchWorkPacket,
        CodeCheckName::ChangedFilesLimited,
        CodeCheckName::NonCodeFilesPreserved,
        CodeCheckName::CheckerStaticSafety,
        CodeCheckName::CheckerFunctionContract,
        CodeCheckName::CheckerSampleCases,
        CodeCheckName::NoNetworkObserved,
        CodeCheckName::NoFilesystemEscapeObserved,
        CodeCheckName::NoSecretEnvPresent,
        CodeCheckName::RepairedDigestComputed,
        CodeCheckName::RawOutputRedacted,
    ]
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|ch| matches!(ch, '0'..='9' | 'a'..='f'))
}

fn validate_safe_location(value: &str) -> Result<(), CodeRepairPolicyError> {
    if value.is_empty()
        || value.len() > 128
        || contains_forbidden_submitted_text(value)
        || looks_like_prompt_or_command(value)
        || looks_like_secret(value)
        || !is_closed_checker_location(value)
    {
        return Err(CodeRepairPolicyError::UnsafeSubmittedText {
            field: "safe_location",
        });
    }

    Ok(())
}

fn validate_safe_message(value: &str) -> Result<(), CodeRepairPolicyError> {
    if value.is_empty()
        || value.len() > 240
        || contains_forbidden_submitted_text(value)
        || !value.chars().all(is_safe_message_char)
        || looks_like_code_snippet(value)
        || looks_like_control_flow_snippet(value)
        || looks_like_prompt_or_command(value)
        || contains_function_call_shape(value)
        || looks_like_bare_domain(value)
        || looks_like_secret(value)
        || !safe_message_uses_closed_evidence_vocabulary(value)
    {
        return Err(CodeRepairPolicyError::UnsafeSubmittedText {
            field: "safe_message",
        });
    }

    Ok(())
}

fn contains_forbidden_submitted_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("file://")
        || lower.contains('/')
        || lower.contains('\\')
        || lower.contains("```")
        || lower.contains('`')
        || lower.contains('<')
        || lower.contains('>')
        || lower.contains('{')
        || lower.contains('}')
        || lower.contains('[')
        || lower.contains(']')
        || lower.contains("/users/")
        || lower.contains("/home/")
        || lower.contains("\\users\\")
        || lower.contains("prompt:")
        || lower.contains("system:")
        || lower.contains("assistant:")
        || lower.contains("tool_call")
        || lower.contains("traceback")
        || lower.contains("stderr")
        || lower.contains("stdout")
        || contains_code_keyword(&lower, "import")
        || contains_code_keyword(&lower, "def")
        || contains_code_keyword(&lower, "class")
        || contains_code_keyword(&lower, "lambda")
        || lower.contains("eval(")
        || lower.contains("exec(")
        || lower.contains("input(")
        || lower.contains("__")
        || lower.contains("os.")
        || lower.contains("subprocess")
}

fn contains_code_keyword(value: &str, keyword: &str) -> bool {
    value.match_indices(keyword).any(|(index, _)| {
        let before = value[..index].chars().next_back();
        let after = value[index + keyword.len()..].chars().next();
        !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char)
    })
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_safe_message_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            ' ' | '.' | ',' | ':' | ';' | '-' | '\'' | '(' | ')' | '!'
        )
}

fn is_closed_checker_location(value: &str) -> bool {
    if let Some(line) = value.strip_prefix("checker.py:line:") {
        return !line.is_empty() && line.chars().all(|ch| ch.is_ascii_digit());
    }

    let Some(function) = value.strip_prefix("checker.py:function:") else {
        return false;
    };
    let mut chars = function.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn looks_like_code_snippet(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("()")
        || lower.contains("):")
        || lower.contains(" = ")
        || lower.contains("==")
        || lower.contains("!=")
        || lower.contains("+=")
        || lower.contains("-=")
}

fn looks_like_control_flow_snippet(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.lines().any(|line| {
        let trimmed = line.trim_start();
        ["if", "for", "while"].iter().any(|keyword| {
            python_like_control_keyword_tail(trimmed, keyword)
                .is_some_and(|tail| tail.trim_end().ends_with(':'))
        }) || python_like_control_keyword_tail(trimmed, "return").is_some_and(|tail| {
            tail.contains(';') || tail.contains('=') || tail.contains('(') || tail.contains('[')
        })
    })
}

fn python_like_control_keyword_tail<'a>(value: &'a str, keyword: &str) -> Option<&'a str> {
    let tail = value.strip_prefix(keyword)?;
    if tail
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(tail)
}

fn contains_function_call_shape(value: &str) -> bool {
    value
        .char_indices()
        .any(|(index, ch)| ch == '(' && immediately_preceded_by_identifier_char(value, index))
}

fn immediately_preceded_by_identifier_char(value: &str, open_paren_index: usize) -> bool {
    value[..open_paren_index]
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
}

fn looks_like_prompt_or_command(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("continue_on_another_runner")
        || lower.contains("ignore previous")
        || lower.contains("follow these")
        || lower.contains("run this")
        || lower.contains("execute")
        || lower.contains("after this report")
        || contains_command_word(&lower, "continue")
        || contains_command_word(&lower, "stop")
        || contains_command_word(&lower, "quarantine")
        || contains_command_word(&lower, "approve")
        || contains_command_word(&lower, "publish")
        || contains_command_word(&lower, "run")
        || contains_command_word(&lower, "execute")
        || contains_command_word(&lower, "ignore")
        || contains_command_word(&lower, "install")
        || contains_command_word(&lower, "fetch")
        || contains_command_word(&lower, "download")
        || contains_command_word(&lower, "open")
        || contains_command_word(&lower, "resume")
        || contains_command_word(&lower, "proceed")
        || contains_command_word(&lower, "retry")
        || contains_command_word(&lower, "restart")
        || contains_command_word(&lower, "handoff")
        || contains_command_word(&lower, "escalate")
        || contains_command_word(&lower, "route")
        || contains_command_word(&lower, "move")
        || contains_command_word(&lower, "transfer")
        || contains_command_word(&lower, "assign")
        || contains_command_word(&lower, "reassign")
        || contains_command_word(&lower, "delegate")
        || contains_command_word(&lower, "carry")
        || contains_command_word(&lower, "send")
        || contains_command_word(&lower, "forward")
        || contains_command_word(&lower, "pass")
        || contains_command_word(&lower, "please")
        || contains_command_word(&lower, "another")
        || contains_command_word(&lower, "different")
        || contains_command_word(&lower, "elsewhere")
        || contains_command_word(&lower, "runner")
        || contains_command_word(&lower, "worker")
        || contains_command_word(&lower, "operator")
        || contains_command_word(&lower, "curator")
        || contains_command_word(&lower, "admin")
        || contains_command_word(&lower, "reviewer")
        || contains_command_word(&lower, "task")
        || contains_command_word(&lower, "attempt")
        || contains_command_word(&lower, "packet")
        || contains_command_word(&lower, "lease")
        || contains_command_word(&lower, "queue")
        || contains_command_word(&lower, "next")
        || contains_command_word(&lower, "new")
        || contains_command_word(&lower, "this")
        || contains_command_word(&lower, "that")
}

fn contains_command_word(value: &str, word: &str) -> bool {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == word)
}

fn safe_message_uses_closed_evidence_vocabulary(value: &str) -> bool {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|token| !token.is_empty())
        .all(|token| token.chars().all(|ch| ch.is_ascii_digit()) || is_safe_evidence_word(token))
}

fn is_safe_evidence_word(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "access"
            | "actual"
            | "answer"
            | "ast"
            | "bundle"
            | "case"
            | "cases"
            | "check"
            | "checker"
            | "code"
            | "computed"
            | "concurrency"
            | "contract"
            | "determinism"
            | "deterministic"
            | "digest"
            | "dynamic"
            | "environment"
            | "escape"
            | "evidence"
            | "expected"
            | "failed"
            | "failure"
            | "file"
            | "files"
            | "filesystem"
            | "finding"
            | "forbidden"
            | "for"
            | "function"
            | "heuristic"
            | "if"
            | "import"
            | "invalid"
            | "maintainability"
            | "manifest"
            | "match"
            | "mismatch"
            | "missing"
            | "network"
            | "no"
            | "non"
            | "not"
            | "observed"
            | "output"
            | "passed"
            | "preserved"
            | "profile"
            | "pure"
            | "raw"
            | "redacted"
            | "reflection"
            | "reported"
            | "result"
            | "return"
            | "returned"
            | "safety"
            | "sample"
            | "satisfied"
            | "sandbox"
            | "schema"
            | "score"
            | "secret"
            | "self"
            | "source"
            | "static"
            | "status"
            | "style"
            | "subprocess"
            | "success"
            | "value"
            | "was"
    )
}

fn looks_like_bare_domain(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [".com", ".org", ".net", ".io", ".dev", ".edu", ".gov"]
        .iter()
        .any(|suffix| lower.contains(suffix))
}

fn looks_like_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("sk-")
        || lower.contains("token=")
        || lower.contains("api_key")
        || lower.contains("password")
}
