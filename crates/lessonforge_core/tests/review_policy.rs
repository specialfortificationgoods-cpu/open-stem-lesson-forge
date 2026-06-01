use std::error::Error;

use lessonforge_core::ids::{ActorId, ArtifactId, RequestId, ReviewId, ReviewTaskId, WorkPacketId};
use lessonforge_core::review::{
    ArtifactPublicationInput, ArtifactVisibility, FindingInput, FindingSeverity, FindingType,
    HumanReviewWorkPacketProof, PublicArtifactLabel, ReviewClaimContext, ReviewContext,
    ReviewOutcome, ReviewPolicyError, ReviewSubmission, ReviewSubmissionResult, ReviewTaskRecord,
    ReviewType, ReviewerProfile, SourceActorLineage, SourceOutputActor, create_review_task,
    derive_public_label, submit_review as submit_review_policy,
};
use lessonforge_core::state::{
    ActorCapability, ActorStatus, ActorType, ArtifactState, ProposedTaskGraphState, RequestState,
    ReviewTaskState, TrustLevel,
};

#[test]
fn machine_validated_artifact_opens_review_task_and_public_label_stays_machine_validated()
-> Result<(), Box<dyn Error>> {
    let task = create_review_task(review_context()?)?;

    assert_eq!(task.state, ReviewTaskState::Open);
    assert_eq!(
        task.review_types,
        vec![ReviewType::SubjectCorrectness, ReviewType::Pedagogy]
    );
    assert_eq!(
        task.required_reviewer_trust_level,
        TrustLevel::ReviewerCandidate
    );
    assert_eq!(
        derive_public_label(ArtifactPublicationInput {
            state: ArtifactState::MachineValidated,
            visibility: ArtifactVisibility::Public,
            previously_public: false,
        }),
        Some(PublicArtifactLabel::MachineValidated)
    );
    Ok(())
}

#[test]
fn review_task_creation_requires_current_parent_state_and_work_packet_dependency_proof()
-> Result<(), Box<dyn Error>> {
    for context in [
        ReviewContext {
            request_state: RequestState::Quarantined,
            ..review_context()?
        },
        ReviewContext {
            request_state: RequestState::Rejected,
            ..review_context()?
        },
        ReviewContext {
            request_state: RequestState::PlanningOpen,
            ..review_context()?
        },
        ReviewContext {
            request_state: RequestState::PlanningInProgress,
            ..review_context()?
        },
        ReviewContext {
            request_state: RequestState::PlanningFailed,
            ..review_context()?
        },
        ReviewContext {
            proposal_state: ProposedTaskGraphState::Superseded,
            ..review_context()?
        },
        ReviewContext {
            human_review_work_packet_proof: HumanReviewWorkPacketProof {
                depends_on_validation_work_packet: false,
                ..HumanReviewWorkPacketProof::valid()
            },
            ..review_context()?
        },
        ReviewContext {
            human_review_work_packet_proof: HumanReviewWorkPacketProof {
                request_lineage_matches: false,
                ..HumanReviewWorkPacketProof::valid()
            },
            ..review_context()?
        },
    ] {
        assert_eq!(
            create_review_task(context)
                .map(|_| "unexpected_ok")
                .unwrap_or_else(|error| error.safe_code()),
            "review_source_state_not_eligible"
        );
    }
    Ok(())
}

#[test]
fn qualified_independent_reviewer_approval_promotes_peer_reviewed() -> Result<(), Box<dyn Error>> {
    let task = create_review_task(review_context()?)?;
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    let lineage = source_lineage()?;
    let result = submit_review(
        task,
        ReviewClaimContext {
            reviewer: reviewer.clone(),
            source_lineage: lineage.clone(),
            lease_active: true,
            artifact_state: ArtifactState::MachineValidated,
            request_state: RequestState::MachineValidated,
            proposal_state: ProposedTaskGraphState::Promoted,
            now: 42,
            trusted_validation_passed: true,
            open_blocking_findings_elsewhere: false,
        },
        ReviewSubmission::approved_no_findings(),
    )?;

    assert_eq!(
        result.review.review_id,
        ReviewId::try_from("review_energy_001")?
    );
    assert_eq!(result.review.reviewer_actor_id, reviewer.reviewer_actor_id);
    assert_eq!(
        result.review.outcome,
        ReviewOutcome::ApprovedForPeerReviewed
    );
    assert_eq!(
        result.review.review_work_packet_id,
        WorkPacketId::try_from("wp_review")?
    );
    assert_eq!(result.review.schema_version, "human-review-record-v1");
    assert_eq!(result.review.scope_id, "scope_default");
    assert_eq!(
        result.review.request_id,
        RequestId::try_from("req_energy_001")?
    );
    assert_eq!(result.review.created_at, 42);
    assert_eq!(result.review.source_lineage, lineage);
    assert_eq!(result.review.recommended_next_state, None);
    assert_eq!(result.review_task_state, ReviewTaskState::Submitted);
    assert_eq!(
        result.system_completed_review_task_state,
        ReviewTaskState::Completed
    );
    assert!(result.findings.is_empty());
    assert_eq!(result.artifact_state, ArtifactState::PeerReviewed);
    assert_eq!(result.public_label, Some(PublicArtifactLabel::PeerReviewed));
    Ok(())
}

#[test]
fn review_submission_requires_claimed_task_not_merely_open_task() -> Result<(), Box<dyn Error>> {
    let open_task = create_review_task(review_context()?)?;
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;

    let result = submit_review_policy(
        open_task,
        review_claim_context(reviewer)?,
        ReviewSubmission::approved_no_findings(),
    );

    assert_eq!(
        result
            .map(|_| "unexpected_ok")
            .unwrap_or_else(|error| error.safe_code()),
        "review_submission_lineage_mismatch"
    );
    Ok(())
}

#[test]
fn review_and_finding_ids_are_derived_from_review_task_identity() -> Result<(), Box<dyn Error>> {
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    let first = submit_review(
        create_review_task(review_context()?)?,
        review_claim_context(reviewer.clone())?,
        submission_with_finding(FindingType::FormattingIssue, FindingSeverity::Note),
    )?;
    let mut second_context = review_context()?;
    second_context.review_task_id = ReviewTaskId::try_from("rtask_energy_002")?;
    second_context.review_work_packet_id = WorkPacketId::try_from("wp_review_002")?;
    second_context.artifact_id = ArtifactId::try_from("art_energy_002")?;
    let second = submit_review(
        create_review_task(second_context)?,
        review_claim_context(reviewer)?,
        submission_with_finding(FindingType::FormattingIssue, FindingSeverity::Note),
    )?;

    assert_ne!(first.review.review_id, second.review.review_id);
    assert_ne!(first.findings[0].finding_id, second.findings[0].finding_id);
    assert_eq!(
        first.review.review_id,
        ReviewId::try_from("review_energy_001")?
    );
    assert_eq!(
        second.review.review_id,
        ReviewId::try_from("review_energy_002")?
    );
    Ok(())
}

#[test]
fn conflicted_or_unqualified_reviewer_cannot_claim_or_approve() -> Result<(), Box<dyn Error>> {
    let task = create_review_task(review_context()?)?;
    for reviewer in [
        qualified_reviewer(
            "actor_planner_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        qualified_reviewer(
            "actor_reviewer_001",
            "operator_generator",
            "conflict_reviewer",
        )?,
        qualified_reviewer(
            "actor_reviewer_002",
            "operator_reviewer",
            "conflict_generator",
        )?,
        ReviewerProfile {
            independence_verified: false,
            ..qualified_reviewer(
                "actor_reviewer_003",
                "operator_reviewer",
                "conflict_reviewer",
            )?
        },
        ReviewerProfile {
            scope_id: "scope_other".to_owned(),
            ..qualified_reviewer(
                "actor_reviewer_004",
                "operator_reviewer",
                "conflict_reviewer",
            )?
        },
        ReviewerProfile {
            review_capabilities: vec![ActorCapability::HumanSubjectReview],
            ..qualified_reviewer(
                "actor_reviewer_005",
                "operator_reviewer",
                "conflict_reviewer",
            )?
        },
        qualified_reviewer(
            "actor_packager_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?,
        qualified_reviewer(
            "actor_reviewer_006",
            "operator_packager",
            "conflict_reviewer",
        )?,
        qualified_reviewer(
            "actor_reviewer_007",
            "operator_reviewer",
            "conflict_packager",
        )?,
    ] {
        assert_eq!(
            review_error_code(
                task.clone(),
                reviewer,
                ReviewSubmission::approved_no_findings()
            ),
            "review_conflict"
        );
    }
    Ok(())
}

#[test]
fn reviewer_subject_and_age_qualification_come_from_review_task_context()
-> Result<(), Box<dyn Error>> {
    let mut chemistry_context = review_context()?;
    chemistry_context.required_subject = "chemistry".to_owned();
    chemistry_context.required_age_range = "16-18".to_owned();
    let chemistry_task = create_review_task(chemistry_context)?;

    assert_eq!(
        review_error_code(
            chemistry_task.clone(),
            qualified_reviewer(
                "actor_reviewer_001",
                "operator_reviewer",
                "conflict_reviewer",
            )?,
            ReviewSubmission::approved_no_findings(),
        ),
        "review_conflict"
    );

    let chemistry_reviewer = ReviewerProfile {
        trusted_subjects: vec!["chemistry".to_owned()],
        trusted_age_ranges: vec!["16-18".to_owned()],
        ..qualified_reviewer(
            "actor_reviewer_002",
            "operator_reviewer_2",
            "conflict_reviewer_2",
        )?
    };
    let result = submit_review(
        chemistry_task,
        review_claim_context(chemistry_reviewer)?,
        ReviewSubmission::approved_no_findings(),
    )?;
    assert_eq!(result.artifact_state, ArtifactState::PeerReviewed);
    Ok(())
}

#[test]
fn missing_source_lineage_identity_metadata_fails_closed() -> Result<(), Box<dyn Error>> {
    let task = create_review_task(review_context()?)?;
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    let base = source_lineage()?;
    for lineage in [
        SourceActorLineage {
            planner_operator_account_id: String::new(),
            ..base.clone()
        },
        SourceActorLineage {
            verifier_conflict_group_id: String::new(),
            ..base.clone()
        },
        SourceActorLineage {
            generator_operator_account_id: String::new(),
            ..base.clone()
        },
        SourceActorLineage {
            accepted_output_actors: vec![SourceOutputActor {
                actor_id: ActorId::try_from("actor_packager_001")?,
                operator_account_id: String::new(),
                conflict_group_id: "conflict_packager".to_owned(),
            }],
            ..base.clone()
        },
        SourceActorLineage {
            accepted_output_actors: vec![SourceOutputActor {
                actor_id: ActorId::try_from("actor_packager_001")?,
                operator_account_id: "operator_packager".to_owned(),
                conflict_group_id: String::new(),
            }],
            ..base.clone()
        },
    ] {
        let result = submit_review(
            task.clone(),
            ReviewClaimContext {
                reviewer: reviewer.clone(),
                source_lineage: lineage,
                lease_active: true,
                artifact_state: ArtifactState::MachineValidated,
                request_state: RequestState::MachineValidated,
                proposal_state: ProposedTaskGraphState::Promoted,
                now: 42,
                trusted_validation_passed: true,
                open_blocking_findings_elsewhere: false,
            },
            ReviewSubmission::approved_no_findings(),
        );
        assert_eq!(
            result
                .map(|_| "unexpected_ok")
                .unwrap_or_else(|error| error.safe_code()),
            "review_conflict"
        );
    }
    Ok(())
}

#[test]
fn blocking_findings_and_authority_fields_prevent_peer_reviewed() -> Result<(), Box<dyn Error>> {
    let task = create_review_task(review_context()?)?;
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    let mut forged = ReviewSubmission::approved_no_findings();
    forged.findings = vec![FindingInput {
        severity: FindingSeverity::Note,
        finding_type: FindingType::LicenseIssue,
        safe_location: "artifact_manifest".to_owned(),
        safe_message: "License metadata needs review".to_owned(),
        submitted_blocking: Some(false),
        submitted_authority_fields: Vec::new(),
    }];
    assert_eq!(
        review_error_code(task.clone(), reviewer.clone(), forged),
        "submitted_finding_authority_field"
    );

    let mut blocking = ReviewSubmission::approved_no_findings();
    blocking.findings = vec![FindingInput {
        severity: FindingSeverity::Major,
        finding_type: FindingType::PhysicsError,
        safe_location: "worksheet".to_owned(),
        safe_message: "Energy value should be checked".to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    let result = submit_review(
        task,
        ReviewClaimContext {
            reviewer,
            source_lineage: source_lineage()?,
            lease_active: true,
            artifact_state: ArtifactState::MachineValidated,
            request_state: RequestState::MachineValidated,
            proposal_state: ProposedTaskGraphState::Promoted,
            now: 42,
            trusted_validation_passed: true,
            open_blocking_findings_elsewhere: false,
        },
        blocking,
    )?;
    assert!(result.findings[0].blocking);
    assert_eq!(result.artifact_state, ArtifactState::MachineValidated);
    assert_eq!(
        result.public_label,
        Some(PublicArtifactLabel::MachineValidated)
    );
    Ok(())
}

#[test]
fn finding_policy_rejects_invalid_severity_unsafe_text_and_laundered_blocking_flags()
-> Result<(), Box<dyn Error>> {
    let task = create_review_task(review_context()?)?;
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;

    let mut always_blocking = ReviewSubmission::approved_no_findings();
    always_blocking.findings = vec![FindingInput {
        severity: FindingSeverity::Note,
        finding_type: FindingType::UnsafeInstruction,
        safe_location: "worksheet".to_owned(),
        safe_message: "Safety note needs review".to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    let result = submit_review(
        task.clone(),
        review_claim_context(reviewer.clone())?,
        always_blocking,
    )?;
    assert!(result.findings[0].blocking);
    assert_eq!(result.artifact_state, ArtifactState::MachineValidated);

    let mut forged_blocking = ReviewSubmission::approved_no_findings();
    forged_blocking.findings = vec![FindingInput {
        severity: FindingSeverity::Note,
        finding_type: FindingType::PiiOrSecretLeak,
        safe_location: "teacher_notes".to_owned(),
        safe_message: "Privacy text needs review".to_owned(),
        submitted_blocking: Some(false),
        submitted_authority_fields: Vec::new(),
    }];
    assert_eq!(
        review_error_code(task.clone(), reviewer.clone(), forged_blocking),
        "submitted_finding_authority_field"
    );

    let mut invalid_severity = ReviewSubmission::approved_no_findings();
    invalid_severity.findings = vec![FindingInput {
        severity: FindingSeverity::Major,
        finding_type: FindingType::FormattingIssue,
        safe_location: "worksheet".to_owned(),
        safe_message: "Layout issue".to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    assert_eq!(
        review_error_code(task.clone(), reviewer.clone(), invalid_severity),
        "invalid_finding"
    );

    let mut unsafe_text = ReviewSubmission::approved_no_findings();
    unsafe_text.findings = vec![FindingInput {
        severity: FindingSeverity::Minor,
        finding_type: FindingType::OtherSafe,
        safe_location: "worksheet".to_owned(),
        safe_message: "See https://unsafe.example".to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    assert_eq!(
        review_error_code(task, reviewer, unsafe_text),
        "unsafe_submitted_review_text"
    );

    for unsafe_value in [
        "/home/alice/.ssh/id_rsa",
        "~/Library/Application Support/app/state",
        r"C:\Users\alice\.codex\auth.json",
        ".codex/auth.json",
        "Student Alice scored 90",
        "student: Alice",
        "student = Alice",
        "grade=90",
        "grade : 90",
        "student roster: Bob Chen",
        "student@example.test",
        "Parent phone 555-123-4567",
        "5551234567x123",
        "call me at 555/123/4567",
        "555/123/4567 phone",
        "phone is 555-1234",
        "contact 44 20 7946 0958",
        "my cell is 555-1234",
        "555-1234 cell",
        "cell phone is 555-1234",
        "cell phone: 555-1234",
        "telephone 555-1234",
        "5551234 phone",
        "+44 20 7946 0958",
        "+1 5551234567",
        "+44 2079460958",
        "phone +1 5551234567",
        "contact 44 2079460958",
        "Charlie Chen: 90%",
        "Charlie Li: 90/100",
        "Quiz score % 85",
        "café score 90/100",
    ] {
        let mut unsafe_path = ReviewSubmission::approved_no_findings();
        unsafe_path.findings = vec![FindingInput {
            severity: FindingSeverity::Minor,
            finding_type: FindingType::OtherSafe,
            safe_location: "worksheet".to_owned(),
            safe_message: unsafe_value.to_owned(),
            submitted_blocking: None,
            submitted_authority_fields: Vec::new(),
        }];
        assert_eq!(
            review_error_code(
                create_review_task(review_context()?)?,
                qualified_reviewer(
                    "actor_reviewer_001",
                    "operator_reviewer",
                    "conflict_reviewer",
                )?,
                unsafe_path,
            ),
            "unsafe_submitted_review_text"
        );
    }

    let mut safe_percentage = ReviewSubmission::approved_no_findings();
    safe_percentage.findings = vec![FindingInput {
        severity: FindingSeverity::Minor,
        finding_type: FindingType::OtherSafe,
        safe_location: "worksheet".to_owned(),
        safe_message: "Efficiency is 85% in the example".to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    submit_review(
        create_review_task(review_context()?)?,
        review_claim_context(qualified_reviewer(
            "actor_reviewer_001",
            "operator_reviewer",
            "conflict_reviewer",
        )?)?,
        safe_percentage,
    )?;

    for safe_value in [
        "Use ISBN 9780131103627 as a source note",
        "Use ISBN 0131103628 as a source note",
        "Use EAN 400 638 1333931 as an example identifier",
        "Compute 1+23456789 using mental math",
        "Compare 1/2 and 3/4 fractions",
        "Use grade 9 examples for introductory physics",
        "Students should compare energy transfers",
        "Use 5551234567 as an opaque example number",
        "Use 15551234567 as an opaque example number",
        "Use 2125550199 as an opaque example number",
        "Use 5551234567extra as a synthetic label",
    ] {
        let mut safe_numeric_text = ReviewSubmission::approved_no_findings();
        safe_numeric_text.findings = vec![FindingInput {
            severity: FindingSeverity::Minor,
            finding_type: FindingType::OtherSafe,
            safe_location: "worksheet".to_owned(),
            safe_message: safe_value.to_owned(),
            submitted_blocking: None,
            submitted_authority_fields: Vec::new(),
        }];
        submit_review(
            create_review_task(review_context()?)?,
            review_claim_context(qualified_reviewer(
                "actor_reviewer_001",
                "operator_reviewer",
                "conflict_reviewer",
            )?)?,
            safe_numeric_text,
        )?;
    }
    Ok(())
}

#[test]
fn nonblocking_findings_may_coexist_with_peer_reviewed_but_changes_requested_does_not_promote()
-> Result<(), Box<dyn Error>> {
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    let mut note = ReviewSubmission::approved_no_findings();
    note.findings = vec![FindingInput {
        severity: FindingSeverity::Note,
        finding_type: FindingType::FormattingIssue,
        safe_location: "teacher_notes".to_owned(),
        safe_message: "Consider adding a timing note".to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    let approved = submit_review(
        create_review_task(review_context()?)?,
        ReviewClaimContext {
            reviewer: reviewer.clone(),
            source_lineage: source_lineage()?,
            lease_active: true,
            artifact_state: ArtifactState::MachineValidated,
            request_state: RequestState::MachineValidated,
            proposal_state: ProposedTaskGraphState::Promoted,
            now: 42,
            trusted_validation_passed: true,
            open_blocking_findings_elsewhere: false,
        },
        note,
    )?;
    assert!(!approved.findings[0].blocking);
    assert_eq!(
        approved.findings[0].parent_type,
        lessonforge_core::review::FindingParentType::Review
    );
    assert_eq!(approved.findings[0].parent_id, approved.review.review_id);
    assert_eq!(
        approved.findings[0].created_by_actor_id,
        reviewer.reviewer_actor_id
    );
    assert_eq!(approved.findings[0].created_at, 42);
    assert_eq!(approved.artifact_state, ArtifactState::PeerReviewed);

    let mut changes = ReviewSubmission::approved_no_findings();
    changes.outcome = ReviewOutcome::ChangesRequested;
    changes.recommended_next_state = Some("peer_reviewed".to_owned());
    let result = submit_review(
        create_review_task(review_context()?)?,
        ReviewClaimContext {
            reviewer,
            source_lineage: source_lineage()?,
            lease_active: true,
            artifact_state: ArtifactState::MachineValidated,
            request_state: RequestState::MachineValidated,
            proposal_state: ProposedTaskGraphState::Promoted,
            now: 42,
            trusted_validation_passed: true,
            open_blocking_findings_elsewhere: false,
        },
        changes,
    )?;
    assert_eq!(result.review.outcome, ReviewOutcome::ChangesRequested);
    assert_eq!(
        result.review.recommended_next_state,
        Some("peer_reviewed".to_owned())
    );
    assert_eq!(result.artifact_state, ArtifactState::MachineValidated);
    Ok(())
}

#[test]
fn review_requested_state_can_promote_but_plan_verification_and_critique_laundering_cannot()
-> Result<(), Box<dyn Error>> {
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    let result = submit_review(
        create_review_task(review_context()?)?,
        ReviewClaimContext {
            artifact_state: ArtifactState::ReviewRequested,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewSubmission::approved_no_findings(),
    )?;
    assert_eq!(result.artifact_state, ArtifactState::PeerReviewed);

    let mut classroom_ready = ReviewSubmission::approved_no_findings();
    classroom_ready.recommended_next_state = Some("classroom_ready".to_owned());
    assert_eq!(
        review_error_code(
            create_review_task(review_context()?)?,
            reviewer.clone(),
            classroom_ready
        ),
        "review_submitted_authority_field"
    );

    let mut plan_verification_laundering = ReviewSubmission::approved_no_findings();
    plan_verification_laundering
        .submitted_authority_fields
        .push("plan_verification_id".to_owned());
    assert_eq!(
        review_error_code(
            create_review_task(review_context()?)?,
            reviewer.clone(),
            plan_verification_laundering
        ),
        "review_submitted_authority_field"
    );

    let mut critique_laundering = ReviewSubmission::approved_no_findings();
    critique_laundering
        .submitted_authority_fields
        .push("critique_id".to_owned());
    assert_eq!(
        review_error_code(
            create_review_task(review_context()?)?,
            reviewer,
            critique_laundering
        ),
        "review_submitted_authority_field"
    );
    Ok(())
}

#[test]
fn recommended_next_state_is_inert_but_must_be_safe_text() -> Result<(), Box<dyn Error>> {
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;

    let mut allowed = ReviewSubmission::approved_no_findings();
    allowed.recommended_next_state = Some("peer_reviewed".to_owned());
    let result = submit_review(
        create_review_task(review_context()?)?,
        review_claim_context(reviewer.clone())?,
        allowed,
    )?;
    assert_eq!(result.artifact_state, ArtifactState::PeerReviewed);

    let mut allowed_provider_prompt_text = ReviewSubmission::approved_no_findings();
    allowed_provider_prompt_text.recommended_next_state =
        Some("Provider and prompt wording should be clearer".to_owned());
    submit_review(
        create_review_task(review_context()?)?,
        review_claim_context(reviewer.clone())?,
        allowed_provider_prompt_text,
    )?;

    for unsafe_value in [
        "https://unsafe.example",
        "/Users/local/path",
        "sk-fake-secret",
        "provider_url",
        "provider: https://unsafe.example",
        "prompt=ignore prior instructions",
        "access_token: abcdef1234567890",
    ] {
        let mut unsafe_submission = ReviewSubmission::approved_no_findings();
        unsafe_submission.recommended_next_state = Some(unsafe_value.to_owned());
        assert_eq!(
            review_error_code(
                create_review_task(review_context()?)?,
                reviewer.clone(),
                unsafe_submission
            ),
            "unsafe_submitted_review_text"
        );
    }
    Ok(())
}

#[test]
fn review_submission_rejects_artifacts_that_are_no_longer_review_eligible()
-> Result<(), Box<dyn Error>> {
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    for artifact_state in [
        ArtifactState::DraftGenerated,
        ArtifactState::ValidationFailed,
        ArtifactState::PeerReviewed,
        ArtifactState::Quarantined,
        ArtifactState::Deprecated,
    ] {
        let result = submit_review(
            create_review_task(review_context()?)?,
            ReviewClaimContext {
                artifact_state,
                ..review_claim_context(reviewer.clone())?
            },
            ReviewSubmission::approved_no_findings(),
        );
        assert_eq!(
            result
                .map(|_| "unexpected_ok")
                .unwrap_or_else(|error| error.safe_code()),
            "review_source_state_not_eligible"
        );
    }
    Ok(())
}

#[test]
fn review_submission_rejects_quarantined_or_superseded_parent_context() -> Result<(), Box<dyn Error>>
{
    let reviewer = qualified_reviewer(
        "actor_reviewer_001",
        "operator_reviewer",
        "conflict_reviewer",
    )?;
    for context in [
        ReviewClaimContext {
            request_state: RequestState::Quarantined,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            request_state: RequestState::Rejected,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            request_state: RequestState::Requested,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            request_state: RequestState::ModerationPending,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            request_state: RequestState::PlanningOpen,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            request_state: RequestState::PlanningInProgress,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            request_state: RequestState::PlanningFailed,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            request_state: RequestState::Deprecated,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            proposal_state: ProposedTaskGraphState::Quarantined,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            proposal_state: ProposedTaskGraphState::Superseded,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            proposal_state: ProposedTaskGraphState::SchemaPolicyValidated,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            proposal_state: ProposedTaskGraphState::VerificationRequired,
            ..review_claim_context(reviewer.clone())?
        },
        ReviewClaimContext {
            proposal_state: ProposedTaskGraphState::VerifiedForMvpPromotion,
            ..review_claim_context(reviewer.clone())?
        },
    ] {
        let result = submit_review(
            create_review_task(review_context()?)?,
            context,
            ReviewSubmission::approved_no_findings(),
        );
        assert_eq!(
            result
                .map(|_| "unexpected_ok")
                .unwrap_or_else(|error| error.safe_code()),
            "review_source_state_not_eligible"
        );
    }
    Ok(())
}

#[test]
fn finding_policy_matrix_matches_spec_for_every_type_and_severity() -> Result<(), Box<dyn Error>> {
    use FindingSeverity::{Critical, Major, Minor, Note};
    use FindingType::{
        AccessibilityNote, CheckerMismatch, FormattingIssue, LicenseIssue, MissingAnswerKey,
        OtherSafe, PedagogyIssue, PhysicsError, PiiOrSecretLeak, UnsafeInstruction,
    };

    let cases = [
        (
            PiiOrSecretLeak,
            vec![Critical, Major, Minor, Note],
            vec![Critical, Major, Minor, Note],
        ),
        (
            UnsafeInstruction,
            vec![Critical, Major, Minor, Note],
            vec![Critical, Major, Minor, Note],
        ),
        (
            LicenseIssue,
            vec![Critical, Major, Minor, Note],
            vec![Critical, Major, Minor, Note],
        ),
        (
            CheckerMismatch,
            vec![Critical, Major, Minor, Note],
            vec![Critical, Major, Minor, Note],
        ),
        (
            MissingAnswerKey,
            vec![Critical, Major, Minor, Note],
            vec![Critical, Major, Minor, Note],
        ),
        (
            PhysicsError,
            vec![Critical, Major, Minor],
            vec![Critical, Major],
        ),
        (PedagogyIssue, vec![Major, Minor, Note], vec![Major]),
        (FormattingIssue, vec![Minor, Note], vec![]),
        (AccessibilityNote, vec![Minor, Note], vec![]),
        (OtherSafe, vec![Minor, Note], vec![]),
    ];

    for (finding_type, allowed_severities, blocking_severities) in cases {
        for severity in [Critical, Major, Minor, Note] {
            let result = submit_review(
                create_review_task(review_context()?)?,
                review_claim_context(qualified_reviewer(
                    "actor_reviewer_001",
                    "operator_reviewer",
                    "conflict_reviewer",
                )?)?,
                submission_with_finding(finding_type, severity),
            );
            if allowed_severities.contains(&severity) {
                let result = result?;
                assert_eq!(result.findings.len(), 1);
                assert_eq!(
                    result.findings[0].blocking,
                    blocking_severities.contains(&severity),
                    "{finding_type:?} {severity:?}"
                );
            } else {
                assert_eq!(
                    result
                        .map(|_| "unexpected_ok")
                        .unwrap_or_else(|error| error.safe_code()),
                    "invalid_finding",
                    "{finding_type:?} {severity:?}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn public_label_mapping_hides_draft_private_quarantined_and_never_public_deprecated() {
    assert_eq!(
        derive_public_label(ArtifactPublicationInput {
            state: ArtifactState::DraftGenerated,
            visibility: ArtifactVisibility::Public,
            previously_public: false,
        }),
        None
    );
    assert_eq!(
        derive_public_label(ArtifactPublicationInput {
            state: ArtifactState::PeerReviewed,
            visibility: ArtifactVisibility::Private,
            previously_public: true,
        }),
        None
    );
    assert_eq!(
        derive_public_label(ArtifactPublicationInput {
            state: ArtifactState::Quarantined,
            visibility: ArtifactVisibility::Public,
            previously_public: true,
        }),
        None
    );
    assert_eq!(
        derive_public_label(ArtifactPublicationInput {
            state: ArtifactState::Deprecated,
            visibility: ArtifactVisibility::Public,
            previously_public: false,
        }),
        None
    );
    assert_eq!(
        derive_public_label(ArtifactPublicationInput {
            state: ArtifactState::Deprecated,
            visibility: ArtifactVisibility::Public,
            previously_public: true,
        }),
        Some(PublicArtifactLabel::Deprecated)
    );
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

fn qualified_reviewer(
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

fn review_claim_context(reviewer: ReviewerProfile) -> Result<ReviewClaimContext, Box<dyn Error>> {
    Ok(ReviewClaimContext {
        reviewer,
        source_lineage: source_lineage()?,
        lease_active: true,
        artifact_state: ArtifactState::MachineValidated,
        request_state: RequestState::MachineValidated,
        proposal_state: ProposedTaskGraphState::Promoted,
        now: 42,
        trusted_validation_passed: true,
        open_blocking_findings_elsewhere: false,
    })
}

fn submission_with_finding(
    finding_type: FindingType,
    severity: FindingSeverity,
) -> ReviewSubmission {
    let mut submission = ReviewSubmission::approved_no_findings();
    submission.findings = vec![FindingInput {
        severity,
        finding_type,
        safe_location: "worksheet".to_owned(),
        safe_message: "Structured review note".to_owned(),
        submitted_blocking: None,
        submitted_authority_fields: Vec::new(),
    }];
    submission
}

fn submit_review(
    task: ReviewTaskRecord,
    context: ReviewClaimContext,
    submission: ReviewSubmission,
) -> Result<ReviewSubmissionResult, ReviewPolicyError> {
    submit_review_policy(
        ReviewTaskRecord {
            state: ReviewTaskState::Claimed,
            ..task
        },
        context,
        submission,
    )
}

fn review_error_code(
    task: lessonforge_core::review::ReviewTaskRecord,
    reviewer: ReviewerProfile,
    submission: ReviewSubmission,
) -> &'static str {
    match submit_review(
        task,
        review_claim_context(reviewer).unwrap_or_else(|_| unreachable!()),
        submission,
    ) {
        Ok(_) => "unexpected_ok",
        Err(error) => error.safe_code(),
    }
}
