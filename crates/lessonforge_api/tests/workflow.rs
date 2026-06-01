use lessonforge_api::workflow::DeterministicWorkflow;
use lessonforge_core::ids::{
    ActorId, ArtifactId, LeaseId, PlanningTaskId, RequestId, RequestModerationReportId,
    RequestModerationTaskId, ReviewTaskId, WorkPacketId,
};
use lessonforge_core::moderation::{
    ModerationCategory, ModerationDecision, ModerationKind, ModerationReportSubmission,
    ModerationSafeReason,
};
use lessonforge_core::request::{
    AutoRepairPreference, IntakeContext, IntakeRejectionReason, RequestWorkflowError,
    StoredRequestVisibility,
};
use lessonforge_core::review::{
    ArtifactVisibility, FindingInput, FindingSeverity, FindingType, HumanReviewWorkPacketProof,
    ReviewContext, ReviewPolicyError, ReviewSubmission, ReviewerProfile, SourceActorLineage,
    SourceOutputActor,
};
use lessonforge_core::state::{
    ActorCapability, ActorStatus, ActorType, ArtifactState, PlanningTaskState,
    ProposedTaskGraphState, RequestState, ReviewTaskState, TrustLevel,
};
use serde_json::json;
use std::error::Error;

#[test]
fn api_workflow_composes_intake_and_moderation_deterministically() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let intake = workflow.submit_request(valid_payload(), intake_context()?)?;

    assert_eq!(intake.request.state, RequestState::ModerationPending);
    assert!(intake.planning_task.is_none());

    let report_without_claim = moderation_report(&intake, "moderation-claim-token")?;
    assert!(
        workflow
            .submit_moderation_report(report_without_claim)
            .is_err()
    );

    workflow.claim_request_moderation_task(
        intake.moderation_task.task_id.clone(),
        LeaseId::try_from("lease_rmoderation_energy_001")?,
        ActorId::try_from("actor_moderator_001")?,
        "moderation-claim-token",
        true,
        true,
    )?;
    let debug_state = format!("{workflow:?}");
    assert!(!debug_state.contains("moderation-claim-token"));
    assert!(!debug_state.contains("claim_token_hash"));
    assert!(
        workflow
            .claim_request_moderation_task(
                intake.moderation_task.task_id.clone(),
                LeaseId::try_from("lease_rmoderation_energy_002")?,
                ActorId::try_from("actor_moderator_002")?,
                "other-moderation-claim-token",
                true,
                true,
            )
            .is_err()
    );
    let wrong_token_report = moderation_report(&intake, "wrong-token")?;
    assert!(
        workflow
            .submit_moderation_report(wrong_token_report)
            .is_err()
    );

    let report = moderation_report(&intake, "moderation-claim-token")?;
    let moderation = workflow.submit_moderation_report(report.clone())?;

    assert_eq!(moderation.request_state, RequestState::PlanningOpen);
    let Some(planning_task) = moderation.planning_task else {
        return Err("planning task should be created".into());
    };
    assert_eq!(
        planning_task.planning_task_id,
        PlanningTaskId::try_from("ptask_server_allocated_777")?
    );
    assert_eq!(planning_task.state, PlanningTaskState::Open);
    assert_eq!(workflow.planning_task_count(), 1);
    assert!(
        workflow
            .claim_request_moderation_task(
                intake.moderation_task.task_id.clone(),
                LeaseId::try_from("lease_rmoderation_energy_003")?,
                ActorId::try_from("actor_moderator_003")?,
                "post-submit-token",
                true,
                true,
            )
            .is_err()
    );

    let replay = workflow.submit_moderation_report(report)?;
    assert_eq!(replay.request_state, RequestState::PlanningOpen);
    assert_eq!(workflow.planning_task_count(), 1);
    Ok(())
}

#[test]
fn api_workflow_moderation_submission_uses_stored_claim_authorization() -> Result<(), Box<dyn Error>>
{
    let mut workflow = test_workflow();
    let intake = workflow.submit_request(valid_payload(), intake_context()?)?;
    workflow.claim_request_moderation_task(
        intake.moderation_task.task_id.clone(),
        LeaseId::try_from("lease_rmoderation_energy_001")?,
        ActorId::try_from("actor_moderator_001")?,
        "moderation-claim-token",
        false,
        true,
    )?;

    let error = workflow
        .submit_moderation_report(moderation_report(&intake, "moderation-claim-token")?)
        .err();

    assert!(matches!(
        error,
        Some(RequestWorkflowError::ModerationRejected {
            reason: "moderation_context_not_authorized"
        })
    ));
    Ok(())
}

#[test]
fn api_workflow_rejects_malformed_auto_repair_preference() -> Result<(), Box<dyn Error>> {
    for value in [json!(123), json!(null), json!({})] {
        let mut workflow = test_workflow();
        let mut payload = valid_payload();
        payload["auto_repair_preference"] = value;

        let error = match workflow.submit_request(payload, intake_context()?) {
            Ok(_) => return Err("malformed auto_repair_preference should reject".into()),
            Err(error) => error,
        };

        match error {
            RequestWorkflowError::Rejected { reason, field_path } => {
                assert_eq!(reason, IntakeRejectionReason::InvalidField);
                assert_eq!(field_path, "/auto_repair_preference");
            }
            _ => return Err("expected request intake rejection".into()),
        }
    }
    Ok(())
}

#[test]
fn api_workflow_rejects_unsupported_auto_repair_preference() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let mut payload = valid_payload();
    payload["auto_repair_preference"] = json!("keep_fixing");

    let error = match workflow.submit_request(payload, intake_context()?) {
        Ok(_) => return Err("unsupported auto_repair_preference should reject".into()),
        Err(error) => error,
    };

    match error {
        RequestWorkflowError::Rejected { reason, field_path } => {
            assert_eq!(reason, IntakeRejectionReason::UnsupportedMvpValue);
            assert_eq!(field_path, "/auto_repair_preference");
        }
        _ => return Err("expected request intake rejection".into()),
    }
    Ok(())
}

#[test]
fn api_workflow_rejects_second_request_without_resetting_state() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let intake = workflow.submit_request(valid_payload(), intake_context()?)?;
    workflow.claim_request_moderation_task(
        intake.moderation_task.task_id.clone(),
        LeaseId::try_from("lease_rmoderation_energy_001")?,
        ActorId::try_from("actor_moderator_001")?,
        "moderation-claim-token",
        true,
        true,
    )?;
    workflow.submit_moderation_report(moderation_report(&intake, "moderation-claim-token")?)?;
    assert_eq!(workflow.planning_task_count(), 1);

    let error = workflow.submit_request(valid_payload(), intake_context()?);

    assert!(matches!(
        error,
        Err(RequestWorkflowError::Conflict {
            reason: "workflow_request_already_active"
        })
    ));
    assert_eq!(workflow.planning_task_count(), 1);
    Ok(())
}

fn moderation_report(
    intake: &lessonforge_core::request::RequestIntakeOutcome,
    claim_token: &str,
) -> Result<ModerationReportSubmission, Box<dyn Error>> {
    Ok(ModerationReportSubmission {
        request_moderation_report_id: RequestModerationReportId::try_from("rmreport_energy_001")?,
        request_moderation_task_id: intake.moderation_task.task_id.clone(),
        request_id: intake.request.request_id.clone(),
        lease_id: LeaseId::try_from("lease_rmoderation_energy_001")?,
        claim_token: claim_token.to_owned(),
        moderation_kind: ModerationKind::DummyFixture,
        decision: ModerationDecision::AllowMvpPlanning,
        category_flags: vec![ModerationCategory::None],
        safe_reason_codes: vec![ModerationSafeReason::ModerationAllowed],
    })
}

#[test]
fn stale_or_changed_moderation_submission_does_not_duplicate_planning() -> Result<(), Box<dyn Error>>
{
    let mut workflow = test_workflow();
    let intake = workflow.submit_request(valid_payload(), intake_context()?)?;
    workflow.claim_request_moderation_task(
        intake.moderation_task.task_id.clone(),
        LeaseId::try_from("lease_rmoderation_energy_001")?,
        ActorId::try_from("actor_moderator_001")?,
        "moderation-claim-token",
        true,
        true,
    )?;
    let report = ModerationReportSubmission {
        request_moderation_report_id: RequestModerationReportId::try_from("rmreport_energy_001")?,
        request_moderation_task_id: intake.moderation_task.task_id.clone(),
        request_id: intake.request.request_id.clone(),
        lease_id: LeaseId::try_from("lease_rmoderation_energy_001")?,
        claim_token: "moderation-claim-token".to_owned(),
        moderation_kind: ModerationKind::DummyFixture,
        decision: ModerationDecision::AllowMvpPlanning,
        category_flags: vec![ModerationCategory::None],
        safe_reason_codes: vec![ModerationSafeReason::ModerationAllowed],
    };

    workflow.submit_moderation_report(report)?;
    let replay_report = ModerationReportSubmission {
        request_moderation_report_id: RequestModerationReportId::try_from("rmreport_energy_002")?,
        request_moderation_task_id: intake.moderation_task.task_id.clone(),
        request_id: intake.request.request_id.clone(),
        lease_id: LeaseId::try_from("lease_rmoderation_energy_001")?,
        claim_token: "moderation-claim-token".to_owned(),
        moderation_kind: ModerationKind::DummyFixture,
        decision: ModerationDecision::AllowMvpPlanning,
        category_flags: vec![ModerationCategory::None],
        safe_reason_codes: vec![ModerationSafeReason::ModerationAllowed],
    };
    workflow.submit_moderation_report(replay_report)?;
    let wrong_token_replay = ModerationReportSubmission {
        request_moderation_report_id: RequestModerationReportId::try_from("rmreport_energy_003")?,
        request_moderation_task_id: intake.moderation_task.task_id.clone(),
        request_id: intake.request.request_id.clone(),
        lease_id: LeaseId::try_from("lease_rmoderation_energy_001")?,
        claim_token: "wrong-moderation-claim-token".to_owned(),
        moderation_kind: ModerationKind::DummyFixture,
        decision: ModerationDecision::AllowMvpPlanning,
        category_flags: vec![ModerationCategory::None],
        safe_reason_codes: vec![ModerationSafeReason::ModerationAllowed],
    };
    assert!(
        workflow
            .submit_moderation_report(wrong_token_replay)
            .is_err()
    );
    let changed_report = ModerationReportSubmission {
        request_moderation_report_id: RequestModerationReportId::try_from("rmreport_energy_004")?,
        request_moderation_task_id: intake.moderation_task.task_id,
        request_id: intake.request.request_id,
        lease_id: LeaseId::try_from("lease_rmoderation_energy_001")?,
        claim_token: "moderation-claim-token".to_owned(),
        moderation_kind: ModerationKind::DummyFixture,
        decision: ModerationDecision::RejectRequest,
        category_flags: vec![ModerationCategory::Privacy],
        safe_reason_codes: vec![ModerationSafeReason::ModerationRejectedPrivacy],
    };
    assert!(workflow.submit_moderation_report(changed_report).is_err());
    assert_eq!(workflow.planning_task_count(), 1);
    Ok(())
}

#[test]
fn api_workflow_review_requires_claim_and_promotes_only_valid_human_review()
-> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let task = workflow.open_review_task(review_context()?)?;
    assert_eq!(task.state, ReviewTaskState::Open);
    assert_eq!(
        workflow.current_artifact_state(),
        Some(ArtifactState::ReviewRequested)
    );

    let primary_reviewer = reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    let submission = ReviewSubmission::approved_no_findings();
    assert!(
        workflow
            .submit_human_review(
                task.review_task_id.clone(),
                LeaseId::try_from("lease_review_energy_001")?,
                "review-claim-token",
                "review-submit-001",
                submission.clone(),
            )
            .is_err()
    );

    let claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        primary_reviewer,
        source_lineage()?,
    )?;
    assert!(claim.claim_token_returned);
    let Some(claim_token) = claim.claim_token.clone() else {
        return Err("first claim should return token".into());
    };
    let debug_state = format!("{workflow:?}");
    assert!(!debug_state.contains(&claim_token));
    assert!(!debug_state.contains("review-claim-001"));
    assert!(!debug_state.contains("claim_token_hash"));
    assert!(!debug_state.contains("test-review-claim-secret"));
    let replayed_claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    assert!(!replayed_claim.claim_token_returned);
    assert!(replayed_claim.claim_token.is_none());
    assert!(!format!("{workflow:?}").contains("review-claim-001"));
    assert!(
        workflow
            .claim_review_task(
                task.review_task_id.clone(),
                LeaseId::try_from("lease_review_energy_002")?,
                "review-claim-002",
                reviewer(
                    "actor_reviewer_002",
                    "operator_reviewer_2",
                    "conflict_reviewer_2"
                )?,
                source_lineage()?,
            )
            .is_err()
    );
    for bad_key in [
        "short",
        "has whitespace",
        "https://unsafe.example/key",
        "token-shaped-key",
    ] {
        assert!(
            workflow
                .claim_review_task(
                    task.review_task_id.clone(),
                    LeaseId::try_from("lease_review_energy_003")?,
                    bad_key,
                    reviewer(
                        "actor_reviewer_003",
                        "operator_reviewer_3",
                        "conflict_reviewer_3"
                    )?,
                    source_lineage()?,
                )
                .is_err()
        );
    }
    let result = workflow.submit_human_review(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        &claim_token,
        "review-submit-001",
        submission.clone(),
    )?;
    assert_eq!(result.artifact_state, ArtifactState::PeerReviewed);
    assert_eq!(
        workflow.current_artifact_state(),
        Some(ArtifactState::PeerReviewed)
    );

    let replay = workflow.submit_human_review(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        &claim_token,
        "review-submit-001",
        submission,
    )?;
    assert_eq!(replay, result);

    let mut changed_submission = ReviewSubmission::approved_no_findings();
    changed_submission.recommended_next_state = Some("machine_validated".to_owned());
    assert!(
        workflow
            .submit_human_review(
                ReviewTaskId::try_from("rtask_energy_001")?,
                LeaseId::try_from("lease_review_energy_001")?,
                &claim_token,
                "review-submit-001",
                changed_submission,
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn api_workflow_review_claim_requires_server_secret() -> Result<(), Box<dyn Error>> {
    let mut workflow = DeterministicWorkflow::new();
    let task = workflow.open_review_task(review_context()?)?;

    let error = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    );

    assert!(matches!(
        error,
        Err(ReviewPolicyError::ReviewVerifierSecretUnavailable)
    ));
    Ok(())
}

#[test]
fn api_workflow_review_rejects_wrong_token_and_conflicted_reviewer() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let task = workflow.open_review_task(review_context()?)?;
    assert!(
        workflow
            .claim_review_task(
                task.review_task_id.clone(),
                LeaseId::try_from("lease_review_energy_001")?,
                "review-claim-001",
                reviewer(
                    "actor_generator_001",
                    "operator_reviewer",
                    "conflict_reviewer"
                )?,
                source_lineage()?,
            )
            .is_err()
    );

    workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    assert!(
        workflow
            .submit_human_review(
                task.review_task_id,
                LeaseId::try_from("lease_review_energy_001")?,
                "wrong-token",
                "review-submit-001",
                ReviewSubmission::approved_no_findings(),
            )
            .is_err()
    );
    assert_eq!(
        workflow.current_artifact_state(),
        Some(ArtifactState::ReviewRequested)
    );
    Ok(())
}

#[test]
fn api_workflow_review_rejects_stale_reopen_after_review_requested_or_peer_reviewed()
-> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let task = workflow.open_review_task(review_context()?)?;
    assert!(workflow.open_review_task(review_context()?).is_err());

    let claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    let Some(claim_token) = claim.claim_token else {
        return Err("first claim should return token".into());
    };
    workflow.submit_human_review(
        task.review_task_id,
        LeaseId::try_from("lease_review_energy_001")?,
        &claim_token,
        "review-submit-001",
        ReviewSubmission::approved_no_findings(),
    )?;
    assert_eq!(
        workflow.current_artifact_state(),
        Some(ArtifactState::PeerReviewed)
    );
    assert!(workflow.open_review_task(review_context()?).is_err());
    Ok(())
}

#[test]
fn api_workflow_review_uses_current_gate_state_at_claim_and_submit() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let mut blocked_before_claim = review_context()?;
    blocked_before_claim.request_state = RequestState::Quarantined;
    workflow.open_review_task(review_context()?)?;
    workflow.set_review_gate_state(blocked_before_claim);
    assert!(
        workflow
            .claim_review_task(
                ReviewTaskId::try_from("rtask_energy_001")?,
                LeaseId::try_from("lease_review_energy_001")?,
                "review-claim-001",
                reviewer(
                    "actor_reviewer_001",
                    "operator_reviewer",
                    "conflict_reviewer"
                )?,
                source_lineage()?,
            )
            .is_err()
    );

    let mut workflow = test_workflow();
    let task = workflow.open_review_task(review_context()?)?;
    let claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    let Some(claim_token) = claim.claim_token else {
        return Err("first claim should return token".into());
    };
    let mut quarantined = review_context()?;
    quarantined.request_state = RequestState::Quarantined;
    quarantined.artifact_state = ArtifactState::Quarantined;
    workflow.set_review_gate_state(quarantined);
    assert!(
        workflow
            .submit_human_review(
                task.review_task_id,
                LeaseId::try_from("lease_review_energy_001")?,
                &claim_token,
                "review-submit-001",
                ReviewSubmission::approved_no_findings(),
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn api_workflow_review_rejects_overlong_claim_token() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let task = workflow.open_review_task(review_context()?)?;
    workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;

    let overlong_token = "a".repeat(1024 * 1024);
    let error = workflow.submit_human_review(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        &overlong_token,
        "review-submit-001",
        ReviewSubmission::approved_no_findings(),
    );

    assert!(matches!(
        error,
        Err(ReviewPolicyError::ReviewLeaseNotActive)
    ));
    Ok(())
}

#[test]
fn api_workflow_review_rejects_invalid_idempotency_key() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    let task = workflow.open_review_task(review_context()?)?;
    let claim_error = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    );
    assert!(matches!(
        claim_error,
        Err(ReviewPolicyError::InvalidIdempotencyKey)
    ));

    let claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    let Some(claim_token) = claim.claim_token else {
        return Err("claim should return token".into());
    };

    let error = workflow.submit_human_review(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        &claim_token,
        "token",
        ReviewSubmission::approved_no_findings(),
    );

    assert!(matches!(
        error,
        Err(ReviewPolicyError::InvalidIdempotencyKey)
    ));

    let empty_error = workflow.submit_human_review(
        task.review_task_id,
        LeaseId::try_from("lease_review_energy_001")?,
        &claim_token,
        "",
        ReviewSubmission::approved_no_findings(),
    );
    assert!(matches!(
        empty_error,
        Err(ReviewPolicyError::InvalidIdempotencyKey)
    ));
    Ok(())
}

#[test]
fn api_workflow_review_rejects_expired_claim_and_separator_ambiguous_changed_replay()
-> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    workflow.set_now(10);
    let task = workflow.open_review_task(review_context()?)?;
    let claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    let Some(claim_token) = claim.claim_token else {
        return Err("first claim should return token".into());
    };
    workflow.set_now(claim.expires_at);
    assert!(
        workflow
            .submit_human_review(
                task.review_task_id.clone(),
                LeaseId::try_from("lease_review_energy_001")?,
                &claim_token,
                "review-submit-001",
                ReviewSubmission::approved_no_findings(),
            )
            .is_err()
    );

    workflow.set_now(claim.expires_at + 1);
    let new_claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_002")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_002",
            "operator_reviewer_2",
            "conflict_reviewer_2",
        )?,
        source_lineage()?,
    )?;
    let Some(new_claim_token) = new_claim.claim_token else {
        return Err("second claim should return token".into());
    };
    assert!(
        workflow
            .claim_review_task(
                task.review_task_id.clone(),
                LeaseId::try_from("lease_review_energy_001")?,
                "review-claim-001",
                reviewer(
                    "actor_reviewer_001",
                    "operator_reviewer",
                    "conflict_reviewer",
                )?,
                source_lineage()?,
            )
            .is_err()
    );
    let first_submission = submission_with_single_note("worksheet|line1", "typo");
    workflow.submit_human_review(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_002")?,
        &new_claim_token,
        "review-submit-002",
        first_submission,
    )?;
    let ambiguous_changed_submission = submission_with_single_note("worksheet", "line1|typo");
    assert!(
        workflow
            .submit_human_review(
                task.review_task_id,
                LeaseId::try_from("lease_review_energy_002")?,
                &new_claim_token,
                "review-submit-002",
                ambiguous_changed_submission,
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn api_workflow_review_claim_expiry_saturates_at_u64_max() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    workflow.set_now(u64::MAX - 10);
    let task = workflow.open_review_task(review_context()?)?;

    let claim = workflow.claim_review_task(
        task.review_task_id,
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;

    assert_eq!(claim.expires_at, u64::MAX);
    Ok(())
}

#[test]
fn api_workflow_review_claim_replays_prune_after_expiry() -> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    workflow.set_now(1);
    let task = workflow.open_review_task(review_context()?)?;
    let mut first_claim = None;

    for index in 0..130 {
        let claim = workflow.claim_review_task(
            task.review_task_id.clone(),
            LeaseId::try_from(format!("lease_review_energy_{index:03}"))?,
            &format!("review-claim-{index:03}"),
            reviewer(
                &format!("actor_reviewer_{index:03}"),
                &format!("operator_reviewer_{index:03}"),
                &format!("conflict_reviewer_{index:03}"),
            )?,
            source_lineage()?,
        )?;
        if index == 0 {
            first_claim = Some(claim.clone());
        }
        workflow.set_now(claim.expires_at.saturating_add(1));
    }

    let Some(first_claim) = first_claim else {
        return Err("first claim should be captured".into());
    };
    let replay = workflow.claim_review_task(
        task.review_task_id,
        LeaseId::try_from("lease_review_energy_000")?,
        "review-claim-000",
        reviewer(
            "actor_reviewer_000",
            "operator_reviewer_000",
            "conflict_reviewer_000",
        )?,
        source_lineage()?,
    )?;

    assert_eq!(replay.lease_id, first_claim.lease_id);
    assert!(replay.expires_at > first_claim.expires_at);
    assert!(replay.claim_token_returned);
    assert!(replay.claim_token.is_some());
    Ok(())
}

#[test]
fn api_workflow_reused_lease_and_claim_key_do_not_resurrect_expired_token()
-> Result<(), Box<dyn Error>> {
    let mut workflow = test_workflow();
    workflow.set_now(10);
    let task = workflow.open_review_task(review_context()?)?;
    let claim = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    let Some(expired_token) = claim.claim_token else {
        return Err("first claim should return token".into());
    };

    workflow.set_now(claim.expires_at + 1);
    let replacement = workflow.claim_review_task(
        task.review_task_id.clone(),
        LeaseId::try_from("lease_review_energy_001")?,
        "review-claim-001",
        reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        source_lineage()?,
    )?;
    let Some(replacement_token) = replacement.claim_token.clone() else {
        return Err("replacement claim should return token".into());
    };
    assert_ne!(expired_token, replacement_token);
    assert!(
        workflow
            .submit_human_review(
                task.review_task_id.clone(),
                LeaseId::try_from("lease_review_energy_001")?,
                &expired_token,
                "review-submit-001",
                ReviewSubmission::approved_no_findings(),
            )
            .is_err()
    );
    workflow.submit_human_review(
        task.review_task_id,
        LeaseId::try_from("lease_review_energy_001")?,
        &replacement_token,
        "review-submit-001",
        ReviewSubmission::approved_no_findings(),
    )?;
    Ok(())
}

fn submission_with_single_note(location: &str, message: &str) -> ReviewSubmission {
    let mut submission = ReviewSubmission::approved_no_findings();
    submission.findings = vec![FindingInput {
        severity: FindingSeverity::Note,
        finding_type: FindingType::FormattingIssue,
        safe_location: location.to_owned(),
        safe_message: message.to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    submission
}

fn valid_payload() -> serde_json::Value {
    json!({
        "title": "Conservation of energy lesson pack",
        "subject": "physics",
        "topic": "conservation_of_energy",
        "age_range": "14-16",
        "language": "en",
        "lesson_duration_minutes": 45,
        "desired_artifacts": ["worksheet", "answer_key", "python_checker", "teacher_notes"],
        "constraints": ["no calculus"],
        "license_preference": "CC-BY-4.0",
        "visibility": "public",
        "forbidden_content_acknowledged": true
    })
}

fn intake_context() -> Result<IntakeContext, Box<dyn Error>> {
    Ok(IntakeContext {
        request_id: RequestId::try_from("req_energy_001")?,
        moderation_task_id: RequestModerationTaskId::try_from("rmtask_energy_001")?,
        planning_task_id: PlanningTaskId::try_from("ptask_server_allocated_777")?,
        scope_id: "scope_default".to_owned(),
        created_by_actor_id: ActorId::try_from("actor_teacher_001")?,
        now: "2026-05-30T00:00:00Z".to_owned(),
        default_auto_repair_preference: AutoRepairPreference::NoAutomatedRepair,
        default_visibility: StoredRequestVisibility::Public,
    })
}

fn test_workflow() -> DeterministicWorkflow {
    DeterministicWorkflow::new_with_review_claim_secret("test-review-claim-secret")
}

fn review_context() -> Result<ReviewContext, Box<dyn Error>> {
    Ok(ReviewContext {
        review_task_id: ReviewTaskId::try_from("rtask_energy_001")?,
        review_work_packet_id: WorkPacketId::try_from("wp_review")?,
        artifact_id: ArtifactId::try_from("art_energy_001")?,
        request_id: RequestId::try_from("req_energy_001")?,
        scope_id: "scope_default".to_owned(),
        artifact_state: ArtifactState::MachineValidated,
        request_state: RequestState::MachineValidated,
        proposal_state: ProposedTaskGraphState::Promoted,
        required_subject: "physics".to_owned(),
        required_age_range: "14-16".to_owned(),
        trusted_validation_passed: true,
        human_review_work_packet_proof: HumanReviewWorkPacketProof::valid(),
        open_blocking_validation_findings: false,
        existing_active_review_task: false,
        visibility: ArtifactVisibility::Public,
    })
}

fn reviewer(
    actor_id: &str,
    operator_account_id: &str,
    conflict_group_id: &str,
) -> Result<ReviewerProfile, Box<dyn Error>> {
    Ok(ReviewerProfile {
        reviewer_actor_id: ActorId::try_from(actor_id)?,
        actor_type: ActorType::HumanReviewer,
        status: ActorStatus::Active,
        scope_id: "scope_default".to_owned(),
        operator_account_id: operator_account_id.to_owned(),
        conflict_group_id: conflict_group_id.to_owned(),
        independence_verified: true,
        review_capabilities: vec![
            ActorCapability::HumanSubjectReview,
            ActorCapability::HumanPedagogyReview,
        ],
        trusted_subjects: vec!["physics".to_owned()],
        trusted_age_ranges: vec!["14-16".to_owned()],
        trust_level: TrustLevel::ReviewerCandidate,
    })
}

fn source_lineage() -> Result<SourceActorLineage, Box<dyn Error>> {
    Ok(SourceActorLineage {
        planner_actor_id: ActorId::try_from("actor_planner_001")?,
        verifier_actor_id: ActorId::try_from("actor_verifier_001")?,
        generator_actor_id: ActorId::try_from("actor_generator_001")?,
        accepted_output_actors: vec![SourceOutputActor {
            actor_id: ActorId::try_from("actor_packager_001")?,
            operator_account_id: "operator_packager".to_owned(),
            conflict_group_id: "conflict_packager".to_owned(),
        }],
        planner_operator_account_id: "operator_planner".to_owned(),
        verifier_operator_account_id: "operator_verifier".to_owned(),
        generator_operator_account_id: "operator_generator".to_owned(),
        planner_conflict_group_id: "conflict_planner".to_owned(),
        verifier_conflict_group_id: "conflict_verifier".to_owned(),
        generator_conflict_group_id: "conflict_generator".to_owned(),
    })
}
