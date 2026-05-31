use lessonforge_core::graph::{
    GraphValidationContext, GraphValidationLedger, PlanVerificationEvidence,
    PlanVerificationOutcome, PromotionContext, PromotionIds, PromotionLedger, ValidationDecision,
    promote_verified_proposal, validate_proposed_task_graph,
    validate_proposed_task_graph_with_ledger,
};
use lessonforge_core::ids::{
    ActorId, PlanVerificationTaskId, PlanningTaskId, PromotionDecisionId, RequestId, WorkPacketId,
};
use lessonforge_core::state::{PlanVerificationTaskState, ProposedTaskGraphState, WorkPacketState};
use serde_json::{Value, json};
use std::error::Error;

#[test]
fn valid_mvp_graph_policy_passes_and_creates_verification_task() -> Result<(), Box<dyn Error>> {
    let outcome = validate_proposed_task_graph(valid_graph(), graph_context()?)?;

    assert_eq!(outcome.decision, ValidationDecision::Accepted);
    assert_eq!(
        outcome.proposal.state(),
        ProposedTaskGraphState::VerificationRequired
    );
    assert_eq!(outcome.proposal.central_risk_level(), "low");
    assert_eq!(outcome.errors, Vec::new());
    let Some(task) = outcome.plan_verification_task else {
        return Err("valid proposal should create a plan verification task".into());
    };
    assert_eq!(task.state, PlanVerificationTaskState::Open);
    assert_eq!(task.verification_type, "plan_schema_policy_cross_check");
    Ok(())
}

#[test]
fn rejected_graphs_create_safe_errors_and_no_verification_task() -> Result<(), Box<dyn Error>> {
    let mut graph = valid_graph();
    graph["unexpected_top_level"] = json!("raw rejected value");

    let outcome = validate_proposed_task_graph(graph, graph_context()?)?;

    assert_eq!(outcome.decision, ValidationDecision::Rejected);
    assert_eq!(
        outcome.proposal.state(),
        ProposedTaskGraphState::SchemaRejected
    );
    assert!(outcome.plan_verification_task.is_none());
    assert_eq!(outcome.errors[0].field_path, "/");
    assert!(!format!("{outcome:?}").contains("raw rejected value"));

    let mut forbidden = valid_graph();
    forbidden["proposed_tasks"][0]["provider"] = json!("raw rejected value");
    let forbidden_outcome = validate_proposed_task_graph(forbidden, graph_context()?)?;
    assert_eq!(
        forbidden_outcome.proposal.state(),
        ProposedTaskGraphState::PolicyRejected
    );
    assert_eq!(
        forbidden_outcome.errors[0].field_path,
        "/proposed_tasks/0/provider"
    );
    assert!(!format!("{forbidden_outcome:?}").contains("raw rejected value"));

    let mut foreign_lineage = valid_graph();
    foreign_lineage["request_id"] = json!("req_foreign_001");
    foreign_lineage["planning_task_id"] = json!("ptask_foreign_001");
    foreign_lineage["planner_runner_id"] = json!("actor_foreign_planner_001");
    foreign_lineage["proposed_tasks"][0]["provider"] = json!("raw rejected value");
    let foreign_lineage_outcome = validate_proposed_task_graph(foreign_lineage, graph_context()?)?;
    let rendered = format!("{foreign_lineage_outcome:?}");
    assert!(!rendered.contains("req_foreign_001"));
    assert!(!rendered.contains("ptask_foreign_001"));
    assert!(!rendered.contains("actor_foreign_planner_001"));
    Ok(())
}

#[test]
fn schema_shape_and_identifier_failures_return_safe_rejections() -> Result<(), Box<dyn Error>> {
    let wrong_type = json!("raw rejected value");
    let outcome = validate_proposed_task_graph(wrong_type, graph_context()?)?;
    assert_eq!(outcome.decision, ValidationDecision::Rejected);
    assert_eq!(
        outcome.proposal.state(),
        ProposedTaskGraphState::SchemaRejected
    );
    assert!(outcome.plan_verification_task.is_none());
    assert!(!format!("{outcome:?}").contains("raw rejected value"));

    let mut nested_unknown = valid_graph();
    nested_unknown["proposed_tasks"][0]["unexpected_nested"] = json!("raw rejected value");
    let nested_outcome = validate_proposed_task_graph(nested_unknown, graph_context()?)?;
    assert_eq!(
        nested_outcome.proposal.state(),
        ProposedTaskGraphState::SchemaRejected
    );
    assert!(!format!("{nested_outcome:?}").contains("raw rejected value"));

    let mut invalid_id = valid_graph();
    invalid_id["request_id"] = json!("req_bad/token");
    let id_outcome = validate_proposed_task_graph(invalid_id, graph_context()?)?;
    assert_eq!(
        id_outcome.proposal.state(),
        ProposedTaskGraphState::SchemaRejected
    );
    assert!(id_outcome.keeps_planning_fanout_open);
    Ok(())
}

#[test]
fn validation_replay_returns_existing_plan_verification_task() -> Result<(), Box<dyn Error>> {
    let mut ledger = GraphValidationLedger::default();
    let first =
        validate_proposed_task_graph_with_ledger(valid_graph(), graph_context()?, &mut ledger)?;
    let mut second_context = graph_context()?;
    second_context.plan_verification_task_id = PlanVerificationTaskId::try_from("pvtask_other")?;
    let second =
        validate_proposed_task_graph_with_ledger(valid_graph(), second_context, &mut ledger)?;

    assert_eq!(first.decision, ValidationDecision::Accepted);
    let Some(first_task) = first.plan_verification_task.as_ref() else {
        return Err("first validation should create a plan verification task".into());
    };
    let Some(second_task) = second.plan_verification_task.as_ref() else {
        return Err("replayed validation should return the existing task".into());
    };
    assert_eq!(
        first_task.plan_verification_task_id,
        second_task.plan_verification_task_id
    );
    assert_eq!(ledger.plan_verification_task_count(), 1);

    let mut changed_same_proposal = valid_graph();
    changed_same_proposal["assumptions"] = json!(["changed after accepted validation"]);
    assert!(
        validate_proposed_task_graph_with_ledger(
            changed_same_proposal,
            graph_context()?,
            &mut ledger
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn dependency_and_human_review_policy_failures_do_not_close_fanout() -> Result<(), Box<dyn Error>> {
    let mut cyclic = valid_graph();
    cyclic["proposed_tasks"][0]["depends_on"] = json!(["human_review"]);

    let cyclic_outcome = validate_proposed_task_graph(cyclic, graph_context()?)?;

    assert_eq!(cyclic_outcome.decision, ValidationDecision::Rejected);
    assert_eq!(
        cyclic_outcome.proposal.state(),
        ProposedTaskGraphState::PolicyRejected
    );
    assert!(cyclic_outcome.plan_verification_task.is_none());
    assert!(cyclic_outcome.keeps_planning_fanout_open);

    let mut missing_review = valid_graph();
    missing_review["human_review_required_for"] = json!([]);
    let missing_review_outcome = validate_proposed_task_graph(missing_review, graph_context()?)?;
    assert_eq!(
        missing_review_outcome.proposal.state(),
        ProposedTaskGraphState::PolicyRejected
    );
    Ok(())
}

#[test]
fn promotion_requires_independent_accepted_verification() -> Result<(), Box<dyn Error>> {
    let outcome = validate_proposed_task_graph(valid_graph(), graph_context()?)?;
    let Some(plan_task) = outcome.plan_verification_task.clone() else {
        return Err("valid graph should create a plan verification task".into());
    };
    let mut ledger = PromotionLedger::default();
    let promotion_context = PromotionContext {
        request_id: RequestId::try_from("req_energy_001")?,
        scope_id: "scope_default".to_owned(),
        planning_task_completed_for_proposal: true,
        request_available_for_promotion: true,
        active_competing_planning_leases: 0,
        verification: None,
        open_blocking_findings: false,
    };

    assert!(
        promote_verified_proposal(
            &outcome.proposal,
            promotion_context,
            promotion_ids()?,
            "idem_promote",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            &mut ledger,
        )
        .is_err()
    );

    let blocking = outcome
        .proposal
        .apply_plan_verification(PlanVerificationEvidence {
            plan_verification_task_id: plan_task.plan_verification_task_id.clone(),
            proposal_id: outcome.proposal.proposal_id().clone(),
            request_id: RequestId::try_from("req_energy_001")?,
            planning_task_id: PlanningTaskId::try_from("ptask_energy_001")?,
            verification_type: "plan_schema_policy_cross_check".to_owned(),
            verifier_lease_active_and_consumed: true,
            verifier_actor_id: ActorId::try_from("actor_verifier_001")?,
            verifier_scope_id: "scope_default".to_owned(),
            outcome: PlanVerificationOutcome::BlockingFindings,
            blocking_findings: true,
            critical_or_major_findings: false,
        })?;
    assert_eq!(
        blocking.state(),
        ProposedTaskGraphState::VerificationBlocked
    );

    let forged = outcome
        .proposal
        .apply_plan_verification(PlanVerificationEvidence {
            plan_verification_task_id: PlanVerificationTaskId::try_from("pvtask_other")?,
            proposal_id: outcome.proposal.proposal_id().clone(),
            request_id: RequestId::try_from("req_energy_001")?,
            planning_task_id: PlanningTaskId::try_from("ptask_energy_001")?,
            verification_type: "plan_schema_policy_cross_check".to_owned(),
            verifier_lease_active_and_consumed: true,
            verifier_actor_id: ActorId::try_from("actor_verifier_001")?,
            verifier_scope_id: "scope_default".to_owned(),
            outcome: PlanVerificationOutcome::NoBlockingFindings,
            blocking_findings: false,
            critical_or_major_findings: false,
        });
    assert!(forged.is_err());
    Ok(())
}

#[test]
fn verified_low_risk_graph_promotes_transactionally_and_replay_is_idempotent()
-> Result<(), Box<dyn Error>> {
    let outcome = validate_proposed_task_graph(valid_graph(), graph_context()?)?;
    let Some(plan_task) = outcome.plan_verification_task.clone() else {
        return Err("valid graph should create a plan verification task".into());
    };
    let verified = outcome
        .proposal
        .apply_plan_verification(PlanVerificationEvidence {
            plan_verification_task_id: plan_task.plan_verification_task_id,
            proposal_id: outcome.proposal.proposal_id().clone(),
            request_id: RequestId::try_from("req_energy_001")?,
            planning_task_id: PlanningTaskId::try_from("ptask_energy_001")?,
            verification_type: "plan_schema_policy_cross_check".to_owned(),
            verifier_lease_active_and_consumed: true,
            verifier_actor_id: ActorId::try_from("actor_verifier_001")?,
            verifier_scope_id: "scope_default".to_owned(),
            outcome: PlanVerificationOutcome::NoBlockingFindings,
            blocking_findings: false,
            critical_or_major_findings: false,
        })?;
    assert_eq!(
        verified.state(),
        ProposedTaskGraphState::VerifiedForMvpPromotion
    );

    let mut ledger = PromotionLedger::default();
    let promotion = promote_verified_proposal(
        &verified,
        PromotionContext {
            request_id: RequestId::try_from("req_energy_001")?,
            scope_id: "scope_default".to_owned(),
            planning_task_completed_for_proposal: true,
            request_available_for_promotion: true,
            active_competing_planning_leases: 0,
            verification: Some(PlanVerificationOutcome::NoBlockingFindings),
            open_blocking_findings: false,
        },
        promotion_ids()?,
        "idem_promote",
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        &mut ledger,
    )?;

    assert_eq!(promotion.state.as_str(), "accepted");
    assert_eq!(promotion.proposal.state(), ProposedTaskGraphState::Promoted);
    assert_eq!(promotion.work_packets.len(), 3);
    assert_eq!(promotion.work_packets[0].state, WorkPacketState::Open);
    assert_eq!(
        promotion.work_packets[1].state,
        WorkPacketState::BlockedByDependency
    );
    assert_eq!(
        promotion.work_packets[2].depends_on_work_packet_ids,
        vec![WorkPacketId::try_from("wp_energy_001_validate_pack")?]
    );

    let replay = promote_verified_proposal(
        &verified,
        PromotionContext {
            request_id: RequestId::try_from("req_energy_001")?,
            scope_id: "scope_default".to_owned(),
            planning_task_completed_for_proposal: true,
            request_available_for_promotion: true,
            active_competing_planning_leases: 0,
            verification: Some(PlanVerificationOutcome::NoBlockingFindings),
            open_blocking_findings: false,
        },
        promotion_ids()?,
        "idem_promote",
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        &mut ledger,
    )?;
    assert_eq!(replay.work_packets.len(), 3);
    assert_eq!(ledger.work_packet_count(), 3);

    assert!(
        promote_verified_proposal(
            &verified,
            PromotionContext {
                request_id: RequestId::try_from("req_energy_001")?,
                scope_id: "scope_default".to_owned(),
                planning_task_completed_for_proposal: true,
                request_available_for_promotion: true,
                active_competing_planning_leases: 0,
                verification: None,
                open_blocking_findings: false,
            },
            promotion_ids()?,
            "idem_promote",
            "caller supplied fingerprint ignored",
            &mut ledger,
        )
        .is_err()
    );
    assert!(
        promote_verified_proposal(
            &verified,
            PromotionContext {
                request_id: RequestId::try_from("req_energy_001")?,
                scope_id: "scope_default".to_owned(),
                planning_task_completed_for_proposal: true,
                request_available_for_promotion: true,
                active_competing_planning_leases: 0,
                verification: Some(PlanVerificationOutcome::NoBlockingFindings),
                open_blocking_findings: true,
            },
            promotion_ids()?,
            "idem_promote",
            "caller supplied fingerprint ignored",
            &mut ledger,
        )
        .is_err()
    );
    assert_eq!(ledger.work_packet_count(), 3);

    let proposal_replay = promote_verified_proposal(
        &verified,
        PromotionContext {
            request_id: RequestId::try_from("req_energy_001")?,
            scope_id: "scope_default".to_owned(),
            planning_task_completed_for_proposal: true,
            request_available_for_promotion: true,
            active_competing_planning_leases: 0,
            verification: Some(PlanVerificationOutcome::NoBlockingFindings),
            open_blocking_findings: false,
        },
        PromotionIds {
            promotion_decision_id: PromotionDecisionId::try_from("promo_other")?,
            work_packet_ids: vec![
                WorkPacketId::try_from("wp_energy_001_generate_pack_2")?,
                WorkPacketId::try_from("wp_energy_001_validate_pack_2")?,
                WorkPacketId::try_from("wp_energy_001_human_review_2")?,
            ],
        },
        "idem_promote_other",
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        &mut ledger,
    )?;
    assert_eq!(proposal_replay.work_packets, promotion.work_packets);
    assert_eq!(ledger.work_packet_count(), 3);

    assert!(
        promote_verified_proposal(
            &verified,
            PromotionContext {
                request_id: RequestId::try_from("req_energy_001")?,
                scope_id: "wrong_scope".to_owned(),
                planning_task_completed_for_proposal: true,
                request_available_for_promotion: true,
                active_competing_planning_leases: 0,
                verification: Some(PlanVerificationOutcome::NoBlockingFindings),
                open_blocking_findings: false,
            },
            promotion_ids()?,
            "idem_wrong_scope",
            "caller supplied fingerprint ignored",
            &mut ledger,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn non_low_risk_and_high_risk_task_types_are_rejected_without_verification()
-> Result<(), Box<dyn Error>> {
    let mut medium = valid_graph();
    medium["proposed_tasks"][0]["risk_level"] = json!("medium");
    let medium_outcome = validate_proposed_task_graph(medium, graph_context()?)?;
    assert_eq!(
        medium_outcome.proposal.state(),
        ProposedTaskGraphState::PolicyRejected
    );
    assert_eq!(medium_outcome.proposal.central_risk_level(), "medium");
    assert!(medium_outcome.plan_verification_task.is_none());

    let mut high = valid_graph();
    high["proposed_tasks"][0]["task_type"] = json!("student_placement_task");
    let high_outcome = validate_proposed_task_graph(high, graph_context()?)?;
    assert_eq!(
        high_outcome.proposal.state(),
        ProposedTaskGraphState::PolicyRejected
    );
    assert_eq!(high_outcome.proposal.central_risk_level(), "high");
    assert!(high_outcome.plan_verification_task.is_none());

    let mut extra_executable = valid_graph();
    extra_executable["proposed_tasks"][0]["outputs"] = json!([
        "worksheet.md",
        "answer_key.md",
        "checker.py",
        "extra.py",
        "teacher_notes.md",
        "manifest.json"
    ]);
    let executable_outcome = validate_proposed_task_graph(extra_executable, graph_context()?)?;
    assert_eq!(
        executable_outcome.proposal.state(),
        ProposedTaskGraphState::PolicyRejected
    );
    assert_eq!(executable_outcome.proposal.central_risk_level(), "medium");
    assert!(executable_outcome.plan_verification_task.is_none());

    let mut invalid_execution_policy = valid_graph();
    invalid_execution_policy["proposed_tasks"][0]["execution_policy"] = json!("execute_shell");
    let policy_outcome = validate_proposed_task_graph(invalid_execution_policy, graph_context()?)?;
    assert_eq!(
        policy_outcome.proposal.state(),
        ProposedTaskGraphState::PolicyRejected
    );
    assert_eq!(policy_outcome.proposal.central_risk_level(), "medium");
    assert!(policy_outcome.plan_verification_task.is_none());
    Ok(())
}

#[test]
fn promotion_rejects_duplicate_work_packet_ids_before_ledger_mutation() -> Result<(), Box<dyn Error>>
{
    let outcome = validate_proposed_task_graph(valid_graph(), graph_context()?)?;
    let Some(plan_task) = outcome.plan_verification_task.clone() else {
        return Err("valid graph should create a plan verification task".into());
    };
    let verified = outcome
        .proposal
        .apply_plan_verification(PlanVerificationEvidence {
            plan_verification_task_id: plan_task.plan_verification_task_id,
            proposal_id: outcome.proposal.proposal_id().clone(),
            request_id: RequestId::try_from("req_energy_001")?,
            planning_task_id: PlanningTaskId::try_from("ptask_energy_001")?,
            verification_type: "plan_schema_policy_cross_check".to_owned(),
            verifier_lease_active_and_consumed: true,
            verifier_actor_id: ActorId::try_from("actor_verifier_001")?,
            verifier_scope_id: "scope_default".to_owned(),
            outcome: PlanVerificationOutcome::NoBlockingFindings,
            blocking_findings: false,
            critical_or_major_findings: false,
        })?;
    let mut ledger = PromotionLedger::default();
    let duplicate_ids = PromotionIds {
        promotion_decision_id: PromotionDecisionId::try_from("promo_energy_001")?,
        work_packet_ids: vec![
            WorkPacketId::try_from("wp_energy_001_generate_pack")?,
            WorkPacketId::try_from("wp_energy_001_generate_pack")?,
            WorkPacketId::try_from("wp_energy_001_human_review")?,
        ],
    };

    assert!(
        promote_verified_proposal(
            &verified,
            PromotionContext {
                request_id: RequestId::try_from("req_energy_001")?,
                scope_id: "scope_default".to_owned(),
                planning_task_completed_for_proposal: true,
                request_available_for_promotion: true,
                active_competing_planning_leases: 0,
                verification: Some(PlanVerificationOutcome::NoBlockingFindings),
                open_blocking_findings: false,
            },
            duplicate_ids,
            "idem_duplicate_ids",
            "caller supplied fingerprint ignored",
            &mut ledger,
        )
        .is_err()
    );
    assert_eq!(ledger.work_packet_count(), 0);
    Ok(())
}

fn graph_context() -> Result<GraphValidationContext, Box<dyn Error>> {
    Ok(GraphValidationContext {
        request_id: RequestId::try_from("req_energy_001")?,
        planning_task_id: PlanningTaskId::try_from("ptask_energy_001")?,
        planner_actor_id: ActorId::try_from("actor_planner_001")?,
        scope_id: "scope_default".to_owned(),
        subject: "physics".to_owned(),
        topic: "conservation_of_energy".to_owned(),
        age_range: "14-16".to_owned(),
        language: "en".to_owned(),
        lesson_duration_minutes: 45,
        plan_verification_task_id: PlanVerificationTaskId::try_from("pvtask_energy_001")?,
    })
}

fn promotion_ids() -> Result<PromotionIds, Box<dyn Error>> {
    Ok(PromotionIds {
        promotion_decision_id: PromotionDecisionId::try_from("promo_energy_001")?,
        work_packet_ids: vec![
            WorkPacketId::try_from("wp_energy_001_generate_pack")?,
            WorkPacketId::try_from("wp_energy_001_validate_pack")?,
            WorkPacketId::try_from("wp_energy_001_human_review")?,
        ],
    })
}

fn valid_graph() -> Value {
    json!({
        "proposal_id": "plan_energy_001_a",
        "request_id": "req_energy_001",
        "planning_task_id": "ptask_energy_001",
        "planner_runner_id": "actor_planner_001",
        "schema_version": "1.0",
        "status": "proposed",
        "source_request_summary": {
            "subject": "physics",
            "topic": "conservation_of_energy",
            "age_range": "14-16",
            "duration_minutes": 45,
            "language": "en"
        },
        "assumptions": [],
        "missing_information": [],
        "proposed_artifacts": [
            {"artifact_type": "worksheet", "priority": "required"},
            {"artifact_type": "answer_key", "priority": "required"},
            {"artifact_type": "python_checker", "priority": "required"},
            {"artifact_type": "teacher_notes", "priority": "required"}
        ],
        "validation_plan": [
            "manifest_schema",
            "required_files",
            "license_metadata",
            "ai_assistance_disclosure",
            "obvious_pii_heuristic",
            "obvious_inappropriate_content_heuristic",
            "python_checker_runs",
            "no_external_network_static"
        ],
        "human_review_required_for": ["peer_reviewed"],
        "proposed_tasks": [
            {
                "local_id": "generate_pack",
                "phase": "initial_generation",
                "task_type": "generate_lesson_pack",
                "subject": "physics",
                "topic": "conservation_of_energy",
                "age_range": "14-16",
                "language": "en",
                "risk_level": "low",
                "required_capabilities": ["stem_pedagogy", "structured_markdown", "basic_python"],
                "outputs": ["worksheet.md", "answer_key.md", "checker.py", "teacher_notes.md", "manifest.json"],
                "validation_required": [
                    "manifest_schema",
                    "required_files",
                    "license_metadata",
                    "ai_assistance_disclosure",
                    "obvious_pii_heuristic",
                    "obvious_inappropriate_content_heuristic",
                    "python_checker_runs",
                    "no_external_network_static"
                ],
                "human_review_required_for": ["peer_reviewed"]
            },
            {
                "local_id": "validate_bundle",
                "phase": "mechanical_validation",
                "task_type": "run_artifact_validation",
                "subject": "physics",
                "topic": "conservation_of_energy",
                "age_range": "14-16",
                "language": "en",
                "risk_level": "low",
                "depends_on": ["generate_pack"],
                "required_capabilities": ["artifact_validation", "python_execution_limited"],
                "outputs": ["validation_report.json"],
                "validation_required": [],
                "human_review_required_for": []
            },
            {
                "local_id": "human_review",
                "phase": "human_review",
                "task_type": "review_subject_and_pedagogy",
                "subject": "physics",
                "topic": "conservation_of_energy",
                "age_range": "14-16",
                "language": "en",
                "risk_level": "low",
                "depends_on": ["validate_bundle"],
                "required_capabilities": ["human_subject_review", "human_pedagogy_review"],
                "outputs": ["review.json"],
                "validation_required": [],
                "human_review_required_for": ["peer_reviewed"]
            }
        ]
    })
}
