use lessonforge_schema::{
    SchemaName, validate_artifact_manifest, validate_mvp_request, validate_plan_verification,
    validate_proposed_task_graph, validate_request_moderation_report, verify_fixture_set,
};
use serde_json::{Value, json};
use std::error::Error;
use std::fs;
use std::path::Path;

#[test]
fn real_mvp_fixture_set_matches_closed_contracts() -> Result<(), Box<dyn Error>> {
    let report = verify_fixture_set(workspace_root())?;

    assert_eq!(report.checked_fixture_count, 5);
    assert_eq!(
        report.checked_schemas,
        vec![
            SchemaName::MvpRequest,
            SchemaName::RequestModerationReport,
            SchemaName::ProposedTaskGraph,
            SchemaName::PlanVerification,
            SchemaName::ArtifactManifest,
        ]
    );
    Ok(())
}

#[test]
fn request_fixture_rejects_unknown_and_raw_unsafe_values_safely() -> Result<(), Box<dyn Error>> {
    let mut request = valid_request();
    request["unexpected"] = json!("raw rejected value");

    let error = match validate_mvp_request(&request) {
        Ok(()) => return Err("unknown field should reject".into()),
        Err(error) => error,
    };

    assert_eq!(error.code, "unknown_field");
    assert_eq!(error.field_path, "/");
    assert!(!format!("{error:?}").contains("raw rejected value"));

    let mut unsafe_request = valid_request();
    unsafe_request["constraints"] = json!(["email teacher@example.test"]);
    let unsafe_error = match validate_mvp_request(&unsafe_request) {
        Ok(()) => return Err("unsafe value should reject".into()),
        Err(error) => error,
    };
    assert_eq!(unsafe_error.code, "unsafe_text_value");

    let mut path_request = valid_request();
    path_request["title"] = json!("Read answers from /Users/alice/secrets.txt");
    let path_error = match validate_mvp_request(&path_request) {
        Ok(()) => return Err("local path text should reject".into()),
        Err(error) => error,
    };
    assert_eq!(path_error.code, "unsafe_text_value");

    for unsafe_title in [
        "Read answers from /etc/passwd",
        "Use /private/var/tmp/cache.txt",
        "Include the student profile",
        "Describe the placement decision",
        "Add the disciplinary record",
    ] {
        let mut unsafe_request = valid_request();
        unsafe_request["title"] = json!(unsafe_title);
        let unsafe_error = match validate_mvp_request(&unsafe_request) {
            Ok(()) => return Err(format!("{unsafe_title} should reject").into()),
            Err(error) => error,
        };
        assert_eq!(unsafe_error.code, "unsafe_text_value");
    }

    let mut long_request = valid_request();
    long_request["title"] = json!("x".repeat(121));
    let length_error = match validate_mvp_request(&long_request) {
        Ok(()) => return Err("overlong title should reject".into()),
        Err(error) => error,
    };
    assert_eq!(length_error.code, "text_too_long");
    Ok(())
}

#[test]
fn proposed_task_graph_fixture_rejects_lineage_and_policy_drift() -> Result<(), Box<dyn Error>> {
    let mut graph_with_advisory_execution_policy = valid_graph();
    graph_with_advisory_execution_policy["proposed_tasks"][0]["execution_policy"] =
        json!("code_generation_only");
    validate_proposed_task_graph(&graph_with_advisory_execution_policy)?;

    let mut graph_with_validation_execution_policy = valid_graph();
    graph_with_validation_execution_policy["proposed_tasks"][1]["execution_policy"] =
        json!("code_generation_only");
    let validation_policy_error =
        match validate_proposed_task_graph(&graph_with_validation_execution_policy) {
            Ok(()) => return Err("execution_policy outside generation task should reject".into()),
            Err(error) => error,
        };
    assert_eq!(validation_policy_error.code, "invalid_execution_policy");

    let mut graph = valid_graph();
    graph["source_request_summary"]["duration_minutes"] = json!(60);

    let error = match validate_proposed_task_graph(&graph) {
        Ok(()) => return Err("summary drift should reject".into()),
        Err(error) => error,
    };

    assert_eq!(error.code, "proposal_request_summary_mismatch");
    assert_eq!(error.field_path, "/source_request_summary/duration_minutes");

    let mut graph_without_review = valid_graph();
    graph_without_review["human_review_required_for"] = json!([]);
    let review_error = match validate_proposed_task_graph(&graph_without_review) {
        Ok(()) => return Err("review gate required".into()),
        Err(error) => error,
    };
    assert_eq!(review_error.code, "missing_human_review_gate");

    let mut duplicate_local_id = valid_graph();
    duplicate_local_id["proposed_tasks"][1]["local_id"] = json!("generate_pack");
    let duplicate_error = match validate_proposed_task_graph(&duplicate_local_id) {
        Ok(()) => return Err("duplicate local_id should reject".into()),
        Err(error) => error,
    };
    assert_eq!(duplicate_error.code, "duplicate_task_local_id");

    let mut bad_dependency = valid_graph();
    bad_dependency["proposed_tasks"][1]["depends_on"] = json!(["validate_bundle"]);
    let dependency_error = match validate_proposed_task_graph(&bad_dependency) {
        Ok(()) => return Err("self dependency should reject".into()),
        Err(error) => error,
    };
    assert_eq!(dependency_error.code, "self_task_dependency");
    Ok(())
}

#[test]
fn report_and_manifest_fixtures_are_closed() -> Result<(), Box<dyn Error>> {
    validate_request_moderation_report(&valid_moderation_report())?;
    validate_plan_verification(&valid_plan_verification())?;
    validate_artifact_manifest(&valid_artifact_manifest())?;

    let plan_schema =
        fs::read_to_string(workspace_root().join("schemas/plan_verification.schema.json"))?;
    assert!(plan_schema.contains("no_arbitrary_prompt"));
    assert!(!plan_schema.contains("^no_arbitrary_[a-z]{6}$"));
    Ok(())
}

#[test]
fn fixture_set_rejects_schema_and_example_drift() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &temp.path().join("examples").join("mvp"),
    )?;

    fs::write(
        temp.path().join("schemas").join("request.schema.json"),
        "{ not json",
    )?;
    let schema_error = match verify_fixture_set(temp.path()) {
        Ok(_) => return Err("invalid schema JSON should reject".into()),
        Err(error) => error,
    };
    assert_eq!(schema_error.code, "schema_json_invalid");

    copy_tree(
        &workspace_root().join("schemas"),
        &temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &temp.path().join("examples").join("mvp"),
    )?;
    let mut graph = valid_graph();
    graph["proposed_tasks"][1]["execution_policy"] = json!("code_generation_only");
    fs::write(
        temp.path()
            .join("examples")
            .join("mvp")
            .join("proposed_task_graph.valid.json"),
        serde_json::to_string_pretty(&graph)?,
    )?;
    let graph_schema_error = match verify_fixture_set(temp.path()) {
        Ok(_) => {
            return Err("schema should reject execution_policy outside generation task".into());
        }
        Err(error) => error,
    };
    assert_eq!(graph_schema_error.code, "fixture_schema_validation_failed");

    copy_tree(
        &workspace_root().join("schemas"),
        &temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &temp.path().join("examples").join("mvp"),
    )?;
    fs::write(
        temp.path()
            .join("examples")
            .join("mvp")
            .join("extra.valid.json"),
        "{}",
    )?;
    let extra_error = match verify_fixture_set(temp.path()) {
        Ok(_) => return Err("extra fixture should reject".into()),
        Err(error) => error,
    };
    assert_eq!(extra_error.code, "unexpected_fixture_file");
    Ok(())
}

fn workspace_root() -> std::path::PathBuf {
    let mut root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();
    root
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn valid_request() -> Value {
    json!({
        "title": "Conservation of energy lesson pack",
        "subject": "physics",
        "topic": "conservation_of_energy",
        "age_range": "14-16",
        "language": "en",
        "lesson_duration_minutes": 45,
        "desired_artifacts": ["worksheet", "answer_key", "python_checker", "teacher_notes"],
        "constraints": [
            "no calculus",
            "include kinetic and gravitational potential energy",
            "include one frictionless ramp problem",
            "include an extension question for stronger students"
        ],
        "license_preference": "CC-BY-4.0",
        "visibility": "public",
        "forbidden_content_acknowledged": true
    })
}

fn valid_moderation_report() -> Value {
    json!({
        "request_moderation_report_id": "rmreport_energy_001",
        "request_moderation_task_id": "rmtask_energy_001",
        "request_id": "req_energy_001",
        "lease_id": "lease_rmoderation_energy_001",
        "moderation_kind": "dummy_fixture",
        "decision": "allow_mvp_planning",
        "category_flags": ["none"],
        "safe_reason_codes": ["moderation_allowed"]
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
        "assumptions": ["Students can substitute values into simple formulas."],
        "missing_information": ["Curriculum standard is not specified."],
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

fn valid_plan_verification() -> Value {
    json!({
        "verification_id": "pverify_energy_001_a",
        "proposal_id": "plan_energy_001_a",
        "verification_task_id": "pvtask_energy_001_a",
        "verifier_runner_id": "runner_dummy_verifier_001",
        "verification_type": "plan_schema_policy_cross_check",
        "status": "submitted",
        "outcome": "no_blocking_findings",
        "findings": [],
        "checked_items": [
            "required_artifacts_present",
            "human_review_gate_present",
            "no_student_grading",
            "no_credential_handling",
            "no_arbitrary_prompt",
            "validation_plan_present"
        ],
        "authority": "advisory_only"
    })
}

fn valid_artifact_manifest() -> Value {
    json!({
        "artifact_id": "art_energy_001",
        "request_id": "req_energy_001",
        "work_packet_ids": ["wp_generate"],
        "title": "Conservation of energy lesson pack",
        "subject": "physics",
        "topic": "conservation_of_energy",
        "age_range": "14-16",
        "language": "en",
        "license": "CC-BY-4.0",
        "status_claim": "draft_generated",
        "ai_assisted": true,
        "contents": ["manifest.json", "worksheet.md", "answer_key.md", "checker.py", "teacher_notes.md"],
        "known_limitations": ["Curriculum standard alignment requires teacher review."]
    })
}
