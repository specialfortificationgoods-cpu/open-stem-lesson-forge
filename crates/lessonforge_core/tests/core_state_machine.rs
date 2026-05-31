use lessonforge_core::ids::{ActorId, LeaseId, PlanningTaskId, RequestId, WorkPacketId};
use lessonforge_core::state::{
    ActorCapability, ActorStatus, ActorType, ArtifactState, Lease, LeaseAction, LeaseEntityType,
    LeaseMutationCommand, LeaseReplayDecision, LeaseState, LeaseSubmission, LeaseSubmissionReplay,
    PlanVerificationTaskState, PlanningLeaseFacts, PlanningTaskState, PlanningTaskTransition,
    ProposedTaskGraphState, ProposedTaskGraphTransition, RequestModerationTaskState, RequestState,
    RequestTransition, ReviewTaskState, TransitionContext, TrustLevel, WorkPacketState,
    WorkPacketTransition,
};
use std::error::Error;

#[test]
fn typed_ids_accept_only_their_prefix_and_safe_suffix() {
    assert!(RequestId::try_from("req_energy_pack").is_ok());
    assert!(RequestId::try_from("req_monkey").is_ok());
    assert!(PlanningTaskId::try_from("ptask_energy_plan").is_ok());
    assert!(WorkPacketId::try_from("wp_checker").is_ok());
    assert!(ActorId::try_from("actor_runner_1").is_ok());
    assert!(LeaseId::try_from("lease_claim_1").is_ok());

    assert!(RequestId::try_from("ptask_energy_pack").is_err());
    assert!(RequestId::try_from("req_").is_err());
    assert!(RequestId::try_from("req_teacher@example.com").is_err());
    assert!(RequestId::try_from("req_/tmp/local-path").is_err());
    assert!(RequestId::try_from("req_sk-proj-secret").is_err());
    assert!(RequestId::try_from("req_sk_live_abc123").is_err());
    assert!(RequestId::try_from("req_demo-sk-proj-abc123").is_err());
    assert!(RequestId::try_from("req_demo_sk_live_abc123").is_err());
    assert!(RequestId::try_from("req_api_key").is_err());
    assert!(RequestId::try_from("req_credentials").is_err());
    assert!(RequestId::try_from("req_apikey").is_err());
    assert!(RequestId::try_from("req_secretkey").is_err());
    assert!(RequestId::try_from("req_bearertoken").is_err());
    assert!(RequestId::try_from("req_clientsecret").is_err());
    assert!(RequestId::try_from("req_idtoken").is_err());
    assert!(RequestId::try_from("req_jwttoken").is_err());
    assert!(ActorId::try_from("actor_sessiontoken").is_err());
    assert!(RequestId::try_from("req_provider_detail").is_err());
    assert!(RequestId::try_from("req_student_alice").is_err());
    assert!(RequestId::try_from("req_api_key_reference").is_err());
    assert!(RequestId::try_from("req_password_reset").is_err());
    assert!(RequestId::try_from("req_oauth_cookie").is_err());
    assert!(RequestId::try_from(format!("req_{}", "a".repeat(97))).is_err());
}

#[test]
fn actor_capability_and_trust_are_separate_closed_enums() {
    let runner = ActorType::Runner;
    let capability = ActorCapability::TaskDecomposition;
    let trust = TrustLevel::PlannerCandidate;

    assert_eq!(runner.as_str(), "runner");
    assert_eq!(capability.as_str(), "task_decomposition");
    assert_eq!(trust.as_str(), "planner_candidate");
    assert_eq!(ActorStatus::Active.as_str(), "active");
    assert_ne!(TrustLevel::PlannerCandidate.as_str(), capability.as_str());
    assert_eq!(
        ActorCapability::StructuredJsonOutput.as_str(),
        "structured_json_output"
    );
    assert_eq!(
        ActorCapability::PythonExecutionLimited.as_str(),
        "python_execution_limited"
    );
}

#[test]
fn request_transitions_follow_spec_005_guards() {
    assert_eq!(
        RequestState::Requested.transition(RequestTransition::DeterministicIntakePassed),
        Ok(RequestState::ModerationPending)
    );
    assert_eq!(
        RequestState::ModerationPending.transition(RequestTransition::ModerationAllowsPlanning),
        Ok(RequestState::ModerationPassed)
    );
    assert_eq!(
        RequestState::ModerationPassed.transition(RequestTransition::CreatePlanningTask),
        Ok(RequestState::PlanningOpen)
    );
    assert_eq!(
        RequestState::PlanningOpen.transition(RequestTransition::PlanningLeaseActive),
        Ok(RequestState::PlanningInProgress)
    );
    assert_eq!(
        RequestState::PlanningOpen.transition(RequestTransition::PlanningFailed),
        Ok(RequestState::PlanningFailed)
    );
    assert_eq!(
        RequestState::PlanningInProgress.transition(RequestTransition::PlanningFailed),
        Ok(RequestState::PlanningFailed)
    );
    assert_eq!(
        RequestState::PlanningInProgress.transition(RequestTransition::ProposalStored),
        Ok(RequestState::PlanProposed)
    );
    assert_eq!(
        RequestState::PlanProposed.transition(RequestTransition::PromotionAccepted),
        Ok(RequestState::Decomposed)
    );

    assert!(
        RequestState::Requested
            .transition(RequestTransition::PromotionAccepted)
            .is_err()
    );
}

#[test]
fn planning_task_transitions_reopen_on_retryable_lease_end() {
    assert_eq!(
        PlanningTaskState::Open.transition(PlanningTaskTransition::Claim),
        Ok(PlanningTaskState::Claimed)
    );
    assert_eq!(
        PlanningTaskState::Claimed.transition(PlanningTaskTransition::Submit),
        Ok(PlanningTaskState::Submitted)
    );
    assert_eq!(
        PlanningTaskState::Submitted.transition(PlanningTaskTransition::Complete),
        Ok(PlanningTaskState::Completed)
    );
    assert!(
        PlanningTaskState::Claimed
            .transition(PlanningTaskTransition::LeaseExpiredOrReleased)
            .is_err()
    );
    assert_eq!(
        PlanningTaskState::Claimed.after_lease_end(PlanningLeaseFacts {
            active_lease_count: 0,
            valid_proposal_submitted: false,
            retry_allowed: true,
        }),
        Ok(PlanningTaskState::Open)
    );
    assert_eq!(
        PlanningTaskState::Claimed.after_lease_end(PlanningLeaseFacts {
            active_lease_count: 1,
            valid_proposal_submitted: false,
            retry_allowed: true,
        }),
        Ok(PlanningTaskState::Claimed)
    );
    assert_eq!(
        PlanningTaskState::Claimed.after_lease_end(PlanningLeaseFacts {
            active_lease_count: 0,
            valid_proposal_submitted: true,
            retry_allowed: true,
        }),
        Ok(PlanningTaskState::Submitted)
    );
    assert_eq!(
        PlanningTaskState::Claimed.after_lease_end(PlanningLeaseFacts {
            active_lease_count: 0,
            valid_proposal_submitted: false,
            retry_allowed: false,
        }),
        Ok(PlanningTaskState::Cancelled)
    );

    assert!(
        PlanningTaskState::Completed
            .transition(PlanningTaskTransition::Claim)
            .is_err()
    );
}

#[test]
fn planning_retry_exhaustion_has_terminal_request_and_task_state() -> Result<(), Box<dyn Error>> {
    let context = TransitionContext {
        event_id: "event_planning_retry_exhausted".to_owned(),
        entity_type: "request".to_owned(),
        entity_id: RequestId::try_from("req_energy_pack")?.to_string(),
        scope_id: "scope_energy".to_owned(),
        actor_id: ActorId::try_from("actor_system_core")?,
        actor_type: ActorType::SystemCore,
        command_id: "cmd_planning_retry_exhausted".to_owned(),
        action: "spoofed_context_action".to_owned(),
        reason_code: "planning_abandoned_retry_limit".to_owned(),
        safe_field_path: Some("/state".to_owned()),
        related_ids: vec!["ptask_energy_plan".to_owned()],
        occurred_at: 200,
    };

    let applied = RequestState::PlanningInProgress
        .apply_transition(RequestTransition::PlanningFailed, context)?;

    assert_eq!(applied.next_state, RequestState::PlanningFailed);
    assert_eq!(applied.event.previous_state, "planning_in_progress");
    assert_eq!(applied.event.next_state, "planning_failed");
    assert_eq!(applied.event.action, "planning_failed");
    assert_eq!(applied.event.reason_code, "planning_abandoned_retry_limit");
    assert_eq!(
        PlanningTaskState::Claimed.after_lease_end(PlanningLeaseFacts {
            active_lease_count: 0,
            valid_proposal_submitted: false,
            retry_allowed: false,
        }),
        Ok(PlanningTaskState::Cancelled)
    );
    Ok(())
}

#[test]
fn surface_specific_claimable_states_use_their_own_transition_entities()
-> Result<(), Box<dyn Error>> {
    assert_eq!(
        RequestModerationTaskState::Open.transition(PlanningTaskTransition::Claim),
        Ok(RequestModerationTaskState::Claimed)
    );
    assert_eq!(
        RequestModerationTaskState::Claimed.transition(PlanningTaskTransition::Complete),
        Ok(RequestModerationTaskState::Completed)
    );
    assert!(
        RequestModerationTaskState::Claimed
            .transition(PlanningTaskTransition::Submit)
            .is_err()
    );
    assert_eq!(
        PlanVerificationTaskState::Open.transition(PlanningTaskTransition::Claim),
        Ok(PlanVerificationTaskState::Claimed)
    );
    assert!(
        PlanVerificationTaskState::Claimed
            .transition(PlanningTaskTransition::Claim)
            .is_err()
    );
    assert_eq!(
        ReviewTaskState::Open.transition(PlanningTaskTransition::Claim),
        Ok(ReviewTaskState::Claimed)
    );
    assert!(
        ReviewTaskState::Claimed
            .transition(PlanningTaskTransition::Claim)
            .is_err()
    );

    let Err(error) = ReviewTaskState::Completed.transition(PlanningTaskTransition::Claim) else {
        return Err("completed review task cannot be claimed".into());
    };
    assert_eq!(error.entity, "review_task");
    Ok(())
}

#[test]
fn proposal_state_source_guards_are_explicit() {
    assert_eq!(
        ProposedTaskGraphState::Proposed.transition(ProposedTaskGraphTransition::SchemaRejected),
        Ok(ProposedTaskGraphState::SchemaRejected)
    );
    assert_eq!(
        ProposedTaskGraphState::Proposed
            .transition(ProposedTaskGraphTransition::SchemaPolicyValidated),
        Ok(ProposedTaskGraphState::SchemaPolicyValidated)
    );
    assert_eq!(
        ProposedTaskGraphState::SchemaPolicyValidated
            .transition(ProposedTaskGraphTransition::RequireVerification),
        Ok(ProposedTaskGraphState::VerificationRequired)
    );
    assert!(
        ProposedTaskGraphState::SchemaPolicyValidated
            .transition(ProposedTaskGraphTransition::Promote)
            .is_err()
    );
    assert!(
        ProposedTaskGraphState::VerificationRequired
            .transition(ProposedTaskGraphTransition::Promote)
            .is_err()
    );
    assert_eq!(
        ProposedTaskGraphState::VerificationRequired
            .transition(ProposedTaskGraphTransition::VerifiedForMvpPromotion),
        Ok(ProposedTaskGraphState::VerifiedForMvpPromotion)
    );
    assert_eq!(
        ProposedTaskGraphState::VerifiedForMvpPromotion
            .transition(ProposedTaskGraphTransition::Promote),
        Ok(ProposedTaskGraphState::Promoted)
    );
    assert_eq!(
        ProposedTaskGraphState::VerificationRequired
            .transition(ProposedTaskGraphTransition::VerificationBlocked),
        Ok(ProposedTaskGraphState::VerificationBlocked)
    );
    assert_eq!(
        ProposedTaskGraphState::SchemaPolicyValidated
            .transition(ProposedTaskGraphTransition::Supersede),
        Ok(ProposedTaskGraphState::Superseded)
    );
    assert!(
        ProposedTaskGraphState::PromotionRejected
            .transition(ProposedTaskGraphTransition::Promote)
            .is_err()
    );
}

#[test]
fn transition_application_emits_safe_event_projection() -> Result<(), Box<dyn Error>> {
    let context = TransitionContext {
        event_id: "event_request_1".to_owned(),
        entity_type: "request".to_owned(),
        entity_id: RequestId::try_from("req_energy_pack")?.to_string(),
        scope_id: "scope_energy".to_owned(),
        actor_id: ActorId::try_from("actor_system_core")?,
        actor_type: ActorType::SystemCore,
        command_id: "cmd_intake_passed".to_owned(),
        action: "spoofed_context_action".to_owned(),
        reason_code: "deterministic_intake_passed".to_owned(),
        safe_field_path: Some("/state".to_owned()),
        related_ids: vec!["rmtask_energy".to_owned()],
        occurred_at: 100,
    };

    let applied = RequestState::Requested
        .apply_transition(RequestTransition::DeterministicIntakePassed, context)?;

    assert_eq!(applied.next_state, RequestState::ModerationPending);
    assert_eq!(applied.event.previous_state, "requested");
    assert_eq!(applied.event.next_state, "moderation_pending");
    assert_eq!(applied.event.reason_code, "deterministic_intake_passed");
    assert_eq!(applied.event.scope_id, "scope_energy");
    assert_eq!(applied.event.actor_type, ActorType::SystemCore);
    assert_eq!(applied.event.action, "deterministic_intake_passed");
    assert_eq!(applied.event.safe_field_path.as_deref(), Some("/state"));
    assert_eq!(applied.event.related_ids, vec!["rmtask_energy"]);
    Ok(())
}

#[test]
fn work_packet_transitions_cover_dependency_submission_interruption_and_cancel() {
    assert_eq!(
        WorkPacketState::BlockedByDependency
            .transition(WorkPacketTransition::DependenciesSatisfied),
        Ok(WorkPacketState::Open)
    );
    assert_eq!(
        WorkPacketState::Open.transition(WorkPacketTransition::Claim),
        Ok(WorkPacketState::Claimed)
    );
    assert_eq!(
        WorkPacketState::Claimed.transition(WorkPacketTransition::Submit),
        Ok(WorkPacketState::Submitted)
    );
    assert_eq!(
        WorkPacketState::Submitted.transition(WorkPacketTransition::AcceptInterruption),
        Ok(WorkPacketState::Interrupted)
    );
    assert_eq!(
        WorkPacketState::Submitted.transition(WorkPacketTransition::AcceptSubmission),
        Ok(WorkPacketState::Accepted)
    );
    assert_eq!(
        WorkPacketState::Interrupted.transition(WorkPacketTransition::Cancel),
        Ok(WorkPacketState::Cancelled)
    );

    assert!(
        WorkPacketState::Open
            .transition(WorkPacketTransition::AcceptSubmission)
            .is_err()
    );
}

#[test]
fn artifact_states_include_public_label_derivation_inputs() {
    assert_eq!(ArtifactState::DraftGenerated.as_str(), "draft_generated");
    assert_eq!(
        ArtifactState::ValidationFailed.as_str(),
        "validation_failed"
    );
    assert_eq!(
        ArtifactState::MachineValidated.as_str(),
        "machine_validated"
    );
    assert_eq!(ArtifactState::ReviewRequested.as_str(), "review_requested");
    assert_eq!(ArtifactState::PeerReviewed.as_str(), "peer_reviewed");
}

#[test]
fn lease_submission_consumes_once_and_replay_requires_same_key_and_payload()
-> Result<(), Box<dyn Error>> {
    let lease_id = LeaseId::try_from("lease_planning_1")?;
    let entity_id = PlanningTaskId::try_from("ptask_energy_plan")?;
    let actor_id = ActorId::try_from("actor_runner_1")?;

    let lease = Lease::active(
        lease_id,
        LeaseEntityType::PlanningTask,
        entity_id.to_string(),
        actor_id.clone(),
        Lease::claim_token_hash("claim-token-secret"),
        100,
        200,
    );
    assert_eq!(lease.state(), LeaseState::Active);

    let wrong_token_submission = LeaseSubmission {
        actor_id: actor_id.clone(),
        entity_type: LeaseEntityType::PlanningTask,
        entity_id: entity_id.to_string(),
        claim_token: "wrong-token".to_owned(),
        now: 150,
        idempotency_key: "idem_wrong".to_owned(),
        payload_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_owned(),
        result_id: "plan_energy".to_owned(),
    };
    assert!(
        lease
            .clone()
            .apply(LeaseAction::Submit(wrong_token_submission))
            .is_err()
    );

    let submission = LeaseSubmission {
        actor_id: actor_id.clone(),
        entity_type: LeaseEntityType::PlanningTask,
        entity_id: entity_id.to_string(),
        claim_token: "claim-token-secret".to_owned(),
        now: 150,
        idempotency_key: "idem_1".to_owned(),
        payload_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_owned(),
        result_id: "plan_energy".to_owned(),
    };
    let consumed = lease.apply(LeaseAction::Submit(submission))?;
    assert_eq!(consumed.state(), LeaseState::Consumed);

    assert_eq!(
        consumed.replay_submission(LeaseSubmissionReplay {
            actor_id: actor_id.clone(),
            entity_type: LeaseEntityType::PlanningTask,
            entity_id: entity_id.to_string(),
            claim_token: "claim-token-secret".to_owned(),
            idempotency_key: "idem_1".to_owned(),
            payload_digest:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
        })?,
        LeaseReplayDecision::ReturnOriginalResult {
            result_id: "plan_energy".to_owned()
        }
    );
    assert_eq!(
        consumed.replay_submission(LeaseSubmissionReplay {
            actor_id: actor_id.clone(),
            entity_type: LeaseEntityType::PlanningTask,
            entity_id: entity_id.to_string(),
            claim_token: "claim-token-secret".to_owned(),
            idempotency_key: "idem_1".to_owned(),
            payload_digest:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
        })?,
        LeaseReplayDecision::RejectChangedReplay
    );
    assert!(
        consumed
            .replay_submission(LeaseSubmissionReplay {
                actor_id: actor_id.clone(),
                entity_type: LeaseEntityType::PlanningTask,
                entity_id: entity_id.to_string(),
                claim_token: "wrong-token".to_owned(),
                idempotency_key: "idem_1".to_owned(),
                payload_digest:
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
            })
            .is_err()
    );
    assert!(
        consumed
            .apply(LeaseAction::Release(LeaseMutationCommand {
                actor_id,
                claim_token: "claim-token-secret".to_owned(),
                now: 160,
                idempotency_key: "idem_release".to_owned(),
                payload_digest:
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                        .to_owned(),
                result_id: "release_result".to_owned(),
            }))
            .is_err()
    );
    Ok(())
}

#[test]
fn lease_slots_heartbeat_and_release_replay_are_token_bound() -> Result<(), Box<dyn Error>> {
    let lease_id = LeaseId::try_from("lease_planning_slot_0")?;
    let entity_id = PlanningTaskId::try_from("ptask_energy_plan")?;
    let actor_id = ActorId::try_from("actor_runner_1")?;
    let lease = Lease::active_with_slot(
        lease_id,
        LeaseEntityType::PlanningTask,
        entity_id.to_string(),
        actor_id.clone(),
        Some(0),
        Lease::claim_token_hash("claim-token-secret"),
        100,
        200,
    );

    assert_eq!(lease.lease_slot(), Some(0));
    let heartbeat = LeaseMutationCommand {
        actor_id: actor_id.clone(),
        claim_token: "claim-token-secret".to_owned(),
        now: 120,
        idempotency_key: "idem_heartbeat".to_owned(),
        payload_digest: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
            .to_owned(),
        result_id: "heartbeat_result".to_owned(),
    };
    let heartbeat_applied = lease
        .clone()
        .apply(LeaseAction::Heartbeat(heartbeat.clone()))?;
    assert_eq!(heartbeat_applied.state(), LeaseState::Active);
    assert_eq!(heartbeat_applied.last_heartbeat_at(), Some(120));
    assert_eq!(
        heartbeat_applied
            .clone()
            .apply(LeaseAction::Heartbeat(heartbeat.clone()))?,
        heartbeat_applied
    );

    let mut changed_heartbeat = heartbeat;
    changed_heartbeat.payload_digest =
        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_owned();
    changed_heartbeat.now = 125;
    assert!(
        heartbeat_applied
            .clone()
            .apply(LeaseAction::Heartbeat(changed_heartbeat))
            .is_err()
    );

    let release = LeaseMutationCommand {
        actor_id,
        claim_token: "claim-token-secret".to_owned(),
        now: 130,
        idempotency_key: "idem_release".to_owned(),
        payload_digest: "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            .to_owned(),
        result_id: "release_result".to_owned(),
    };
    let released = heartbeat_applied
        .clone()
        .apply(LeaseAction::Release(release.clone()))?;
    assert_eq!(released.state(), LeaseState::Released);
    assert_eq!(
        released
            .clone()
            .apply(LeaseAction::Release(LeaseMutationCommand {
                now: 250,
                ..release
            }))?,
        released
    );

    let revoke_lease = Lease::active_with_slot(
        LeaseId::try_from("lease_planning_slot_1")?,
        LeaseEntityType::PlanningTask,
        entity_id.to_string(),
        ActorId::try_from("actor_runner_2")?,
        Some(1),
        Lease::claim_token_hash("claim-token-two"),
        100,
        200,
    );
    let revoked = revoke_lease.apply(LeaseAction::Revoke(LeaseMutationCommand {
        actor_id: ActorId::try_from("actor_runner_2")?,
        claim_token: "claim-token-two".to_owned(),
        now: 140,
        idempotency_key: "idem_revoke".to_owned(),
        payload_digest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            .to_owned(),
        result_id: "revoke_result".to_owned(),
    }))?;
    assert_eq!(revoked.state(), LeaseState::Revoked);
    Ok(())
}
