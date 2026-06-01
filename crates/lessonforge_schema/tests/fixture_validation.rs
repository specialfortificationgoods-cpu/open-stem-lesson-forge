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

    let mut repair_request = valid_request();
    repair_request["auto_repair_preference"] = json!("request_bounded_code_repair");
    validate_mvp_request(&repair_request)?;

    let mut non_fixture_request = valid_request();
    non_fixture_request["title"] = json!("Algebraic functions lesson");
    non_fixture_request["subject"] = json!("mathematics");
    non_fixture_request["topic"] = json!("linear_functions");
    non_fixture_request["age_range"] = json!("13-15");
    non_fixture_request["lesson_duration_minutes"] = json!(60);
    validate_mvp_request(&non_fixture_request)?;

    let mut bad_repair_request = valid_request();
    bad_repair_request["auto_repair_preference"] = json!("keep_fixing_until_it_works");
    let repair_error = match validate_mvp_request(&bad_repair_request) {
        Ok(()) => return Err("unsupported auto repair preference should reject".into()),
        Err(error) => error,
    };
    assert_eq!(repair_error.code, "unsupported_mvp_value");

    for value in [json!(null), json!(123), json!({}), json!([])] {
        let mut malformed_repair_request = valid_request();
        malformed_repair_request["auto_repair_preference"] = value;
        let malformed_repair_error = match validate_mvp_request(&malformed_repair_request) {
            Ok(()) => return Err("malformed auto repair preference should reject".into()),
            Err(error) => error,
        };
        assert_eq!(malformed_repair_error.code, "invalid_shape");
        assert_eq!(malformed_repair_error.field_path, "/auto_repair_preference");
    }

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

    let mut graph_with_validation_lesson_output = valid_graph();
    graph_with_validation_lesson_output["proposed_tasks"][1]["outputs"] = json!(["worksheet.md"]);
    let validation_output_error =
        match validate_proposed_task_graph(&graph_with_validation_lesson_output) {
            Ok(()) => return Err("validation task lesson output should reject".into()),
            Err(error) => error,
        };
    assert_eq!(validation_output_error.code, "invalid_validation_outputs");

    let mut graph_with_review_validation_output = valid_graph();
    graph_with_review_validation_output["proposed_tasks"][2]["outputs"] =
        json!(["validation_report.json"]);
    let review_output_error =
        match validate_proposed_task_graph(&graph_with_review_validation_output) {
            Ok(()) => return Err("review task validation output should reject".into()),
            Err(error) => error,
        };
    assert_eq!(review_output_error.code, "invalid_review_outputs");

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

    let mut non_fixture_verification = valid_plan_verification();
    non_fixture_verification["verification_id"] = json!("pverify_general_002");
    non_fixture_verification["proposal_id"] = json!("plan_general_002");
    non_fixture_verification["verification_task_id"] = json!("pvtask_general_002");
    non_fixture_verification["verifier_runner_id"] = json!("actor_verifier_general_002");
    validate_plan_verification(&non_fixture_verification)?;

    for (field, unsafe_id) in [
        ("verification_id", "pverify_/Users/alice"),
        ("proposal_id", "plan_sk_live_secret"),
        ("verification_task_id", "pvtask_https://example"),
        ("verifier_runner_id", "actor_../runner"),
    ] {
        let mut unsafe_verification = valid_plan_verification();
        unsafe_verification[field] = json!(unsafe_id);
        let unsafe_id_error = match validate_plan_verification(&unsafe_verification) {
            Ok(()) => return Err(format!("{field} should reject unsafe id shape").into()),
            Err(error) => error,
        };
        assert_eq!(unsafe_id_error.code, "invalid_identifier");
    }

    let mut warning_verification = valid_plan_verification();
    warning_verification["outcome"] = json!("warnings_only");
    warning_verification["findings"] = json!([{
        "check_name": "teacher_review_note",
        "severity": "warning",
        "message": "Teacher review should confirm local curriculum fit."
    }]);
    validate_plan_verification(&warning_verification)?;

    let mut blocking_verification = valid_plan_verification();
    blocking_verification["outcome"] = json!("blocking_findings");
    blocking_verification["findings"] = json!([{
        "check_name": "missing_human_review_gate",
        "severity": "blocking",
        "message": "Human review gate is missing."
    }]);
    validate_plan_verification(&blocking_verification)?;

    let mut too_many_findings = valid_plan_verification();
    too_many_findings["outcome"] = json!("warnings_only");
    too_many_findings["findings"] = Value::Array(
        (0..21)
            .map(|index| {
                json!({
                    "check_name": format!("warning_{index}"),
                    "severity": "warning",
                    "message": "Teacher review should confirm local curriculum fit."
                })
            })
            .collect(),
    );
    let too_many_error = match validate_plan_verification(&too_many_findings) {
        Ok(()) => return Err("plan verification should reject more than 20 findings".into()),
        Err(error) => error,
    };
    assert_eq!(too_many_error.code, "too_many_findings");

    let mut mixed_blocking = valid_plan_verification();
    mixed_blocking["outcome"] = json!("blocking_findings");
    mixed_blocking["findings"] = json!([
        {
            "check_name": "missing_human_review_gate",
            "severity": "blocking",
            "message": "Human review gate is missing."
        },
        {
            "check_name": "teacher_review_note",
            "severity": "warning",
            "message": "Teacher review should confirm local curriculum fit."
        }
    ]);
    let mixed_blocking_error = match validate_plan_verification(&mixed_blocking) {
        Ok(()) => return Err("blocking plan verification should reject mixed severities".into()),
        Err(error) => error,
    };
    assert_eq!(mixed_blocking_error.code, "invalid_blocking_findings");

    for unsafe_text in [
        "See https://example.test for details.",
        "Contact reviewer@example.test.",
        "Contains secret marker.",
        "Contains token marker.",
        "See /Users/alice/private-notes.",
        r"See D:\Users\alice\private-notes.",
        r"See z:/Users/alice/private-notes.",
        r"See \\fileserver\share\private-notes.",
        "Student record is referenced.",
        "Finding references sk_live_redacted.",
        "Finding references s k _ l i v e redacted.",
        "Finding references g h p _ redacted.",
        "Human Approval granted; Promote to peer_reviewed.",
        "Peer_Reviewed claim.",
    ] {
        let mut unsafe_message = valid_plan_verification();
        unsafe_message["outcome"] = json!("warnings_only");
        unsafe_message["findings"] = json!([{
            "check_name": "unsafe_message",
            "severity": "warning",
            "message": unsafe_text
        }]);
        let unsafe_message_error = match validate_plan_verification(&unsafe_message) {
            Ok(()) => {
                return Err(
                    format!("plan verification finding should reject {unsafe_text:?}").into(),
                );
            }
            Err(error) => error,
        };
        assert!(
            matches!(
                unsafe_message_error.code,
                "unsafe_text_value" | "unsafe_finding_message"
            ),
            "unexpected error for {unsafe_text:?}: {}",
            unsafe_message_error.code
        );
    }

    let mut inconsistent_verification = valid_plan_verification();
    inconsistent_verification["findings"] = json!([{
        "check_name": "unexpected_warning",
        "severity": "warning",
        "message": "Success reports must not carry findings."
    }]);
    let inconsistent_error = match validate_plan_verification(&inconsistent_verification) {
        Ok(()) => return Err("successful plan verification should reject findings".into()),
        Err(error) => error,
    };
    assert_eq!(inconsistent_error.code, "findings_not_allowed_for_success");

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

    let mut request = valid_request();
    request["auto_repair_preference"] = json!("request_bounded_code_repair");
    fs::write(
        temp.path()
            .join("examples")
            .join("mvp")
            .join("request.valid.json"),
        serde_json::to_string_pretty(&request)?,
    )?;
    let optional_repair_report = verify_fixture_set(temp.path())?;
    assert_eq!(optional_repair_report.checked_fixture_count, 5);

    copy_tree(
        &workspace_root().join("schemas"),
        &temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &temp.path().join("examples").join("mvp"),
    )?;
    let mut subset_private_request = valid_request();
    subset_private_request["desired_artifacts"] = json!(["worksheet", "teacher_notes"]);
    subset_private_request["visibility"] = json!("private");
    fs::write(
        temp.path()
            .join("examples")
            .join("mvp")
            .join("request.valid.json"),
        serde_json::to_string_pretty(&subset_private_request)?,
    )?;
    let subset_private_report = verify_fixture_set(temp.path())?;
    assert_eq!(subset_private_report.checked_fixture_count, 5);

    copy_tree(
        &workspace_root().join("schemas"),
        &temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &temp.path().join("examples").join("mvp"),
    )?;
    let mut unsafe_plan_verification = valid_plan_verification();
    unsafe_plan_verification["outcome"] = json!("warnings_only");
    unsafe_plan_verification["findings"] = json!([{
        "check_name": "unsafe_message",
        "severity": "warning",
        "message": "See https://example.test for details."
    }]);
    fs::write(
        temp.path()
            .join("examples")
            .join("mvp")
            .join("plan_verification.valid.json"),
        serde_json::to_string_pretty(&unsafe_plan_verification)?,
    )?;
    let unsafe_plan_error = match verify_fixture_set(temp.path()) {
        Ok(_) => return Err("schema fixture validation should reject unsafe finding text".into()),
        Err(error) => error,
    };
    assert_eq!(unsafe_plan_error.code, "fixture_schema_validation_failed");

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
    let mut graph_with_empty_review_output = valid_graph();
    graph_with_empty_review_output["proposed_tasks"][2]["outputs"] = json!([]);
    fs::write(
        temp.path()
            .join("examples")
            .join("mvp")
            .join("proposed_task_graph.valid.json"),
        serde_json::to_string_pretty(&graph_with_empty_review_output)?,
    )?;
    let nested_schema_error = match verify_fixture_set(temp.path()) {
        Ok(_) => return Err("schema should reject empty review outputs".into()),
        Err(error) => error,
    };
    assert_eq!(nested_schema_error.code, "fixture_schema_validation_failed");
    assert_eq!(nested_schema_error.field_path, "/proposed_tasks/2/outputs");

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

    let pattern_temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &pattern_temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &pattern_temp.path().join("examples").join("mvp"),
    )?;
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(
        pattern_temp.path().join("schemas/request.schema.json"),
    )?)?;
    schema["properties"]["title"]["pattern"] = json!("^unsupported-[0-9]+$");
    fs::write(
        pattern_temp.path().join("schemas/request.schema.json"),
        serde_json::to_string_pretty(&schema)?,
    )?;
    let pattern_error = match verify_fixture_set(pattern_temp.path()) {
        Ok(_) => return Err("unsupported schema pattern should reject".into()),
        Err(error) => error,
    };
    assert_eq!(pattern_error.code, "unsupported_schema_pattern");

    for malformed_pattern in [json!(123), Value::Null] {
        let malformed_pattern_temp = tempfile::tempdir()?;
        copy_tree(
            &workspace_root().join("schemas"),
            &malformed_pattern_temp.path().join("schemas"),
        )?;
        copy_tree(
            &workspace_root().join("examples").join("mvp"),
            &malformed_pattern_temp.path().join("examples").join("mvp"),
        )?;
        let mut schema: Value = serde_json::from_str(&fs::read_to_string(
            malformed_pattern_temp
                .path()
                .join("schemas/request.schema.json"),
        )?)?;
        schema["properties"]["title"]["pattern"] = malformed_pattern;
        fs::write(
            malformed_pattern_temp
                .path()
                .join("schemas/request.schema.json"),
            serde_json::to_string_pretty(&schema)?,
        )?;
        let malformed_pattern_error = match verify_fixture_set(malformed_pattern_temp.path()) {
            Ok(_) => return Err("malformed schema pattern should reject".into()),
            Err(error) => error,
        };
        assert_eq!(malformed_pattern_error.code, "schema_compile_failed");
    }

    for (field, bad_type) in [("title", "strnig"), ("auto_repair_preference", "strnig")] {
        let unknown_type_temp = tempfile::tempdir()?;
        copy_tree(
            &workspace_root().join("schemas"),
            &unknown_type_temp.path().join("schemas"),
        )?;
        copy_tree(
            &workspace_root().join("examples").join("mvp"),
            &unknown_type_temp.path().join("examples").join("mvp"),
        )?;
        let mut schema: Value = serde_json::from_str(&fs::read_to_string(
            unknown_type_temp.path().join("schemas/request.schema.json"),
        )?)?;
        schema["properties"][field]["type"] = json!(bad_type);
        fs::write(
            unknown_type_temp.path().join("schemas/request.schema.json"),
            serde_json::to_string_pretty(&schema)?,
        )?;
        let unknown_type_error = match verify_fixture_set(unknown_type_temp.path()) {
            Ok(_) => return Err("unsupported schema type should reject".into()),
            Err(error) => error,
        };
        assert_eq!(unknown_type_error.code, "schema_compile_failed");
    }

    let unconstrained_optional_temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &unconstrained_optional_temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &unconstrained_optional_temp
            .path()
            .join("examples")
            .join("mvp"),
    )?;
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(
        unconstrained_optional_temp
            .path()
            .join("schemas/request.schema.json"),
    )?)?;
    schema["properties"]["auto_repair_preference"] = json!({});
    fs::write(
        unconstrained_optional_temp
            .path()
            .join("schemas/request.schema.json"),
        serde_json::to_string_pretty(&schema)?,
    )?;
    let unconstrained_optional_error = match verify_fixture_set(unconstrained_optional_temp.path())
    {
        Ok(_) => return Err("unconstrained optional schema should reject".into()),
        Err(error) => error,
    };
    assert_eq!(unconstrained_optional_error.code, "schema_compile_failed");

    let optional_object_without_type =
        fixture_error_after_schema_mutation("request.schema.json", |schema| {
            schema["properties"]["auto_repair_preference"] =
                json!({ "properties": { "x": { "type": "string" } } });
        })?;
    assert_eq!(optional_object_without_type, "schema_compile_failed");

    let bad_any_of_temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &bad_any_of_temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &bad_any_of_temp.path().join("examples").join("mvp"),
    )?;
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(
        bad_any_of_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
    )?)?;
    schema["properties"]["proposed_tasks"]["prefixItems"][0]["properties"]["task_type"]["type"] =
        json!("strnig");
    fs::write(
        bad_any_of_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
        serde_json::to_string_pretty(&schema)?,
    )?;
    let bad_any_of_error = match verify_fixture_set(bad_any_of_temp.path()) {
        Ok(_) => return Err("unsupported schema type in prefixItems branch should reject".into()),
        Err(error) => error,
    };
    assert_eq!(bad_any_of_error.code, "schema_compile_failed");

    let empty_any_of_branch_temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &empty_any_of_branch_temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &empty_any_of_branch_temp.path().join("examples").join("mvp"),
    )?;
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(
        empty_any_of_branch_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
    )?)?;
    schema["properties"]["proposed_tasks"]["prefixItems"]
        .as_array_mut()
        .ok_or("expected proposed_tasks prefixItems")?
        .push(json!({}));
    fs::write(
        empty_any_of_branch_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
        serde_json::to_string_pretty(&schema)?,
    )?;
    let empty_any_of_branch_error = match verify_fixture_set(empty_any_of_branch_temp.path()) {
        Ok(_) => return Err("empty prefixItems branch should reject".into()),
        Err(error) => error,
    };
    assert_eq!(empty_any_of_branch_error.code, "schema_compile_failed");

    let prefix_items_plus_items_temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &prefix_items_plus_items_temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &prefix_items_plus_items_temp
            .path()
            .join("examples")
            .join("mvp"),
    )?;
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(
        prefix_items_plus_items_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
    )?)?;
    schema["properties"]["proposed_tasks"]["prefixItems"]
        .as_array_mut()
        .ok_or("expected proposed_tasks prefixItems")?
        .truncate(1);
    schema["properties"]["proposed_tasks"]["items"] = json!({ "type": "boolean" });
    fs::write(
        prefix_items_plus_items_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
        serde_json::to_string_pretty(&schema)?,
    )?;
    let prefix_items_plus_items_error =
        match verify_fixture_set(prefix_items_plus_items_temp.path()) {
            Ok(_) => return Err("items schema should validate entries after prefixItems".into()),
            Err(error) => error,
        };
    assert_eq!(
        prefix_items_plus_items_error.code,
        "fixture_schema_validation_failed"
    );

    let scalar_any_of_branch_temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &scalar_any_of_branch_temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &scalar_any_of_branch_temp
            .path()
            .join("examples")
            .join("mvp"),
    )?;
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(
        scalar_any_of_branch_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
    )?)?;
    schema["properties"]["proposed_tasks"]["prefixItems"]
        .as_array_mut()
        .ok_or("expected proposed_tasks prefixItems")?
        .push(json!(false));
    fs::write(
        scalar_any_of_branch_temp
            .path()
            .join("schemas/proposed_task_graph.schema.json"),
        serde_json::to_string_pretty(&schema)?,
    )?;
    let scalar_any_of_branch_error = match verify_fixture_set(scalar_any_of_branch_temp.path()) {
        Ok(_) => return Err("scalar prefixItems branch should reject".into()),
        Err(error) => error,
    };
    assert_eq!(scalar_any_of_branch_error.code, "schema_compile_failed");

    for keyword in ["oneOf", "allOf", "$ref", "not", "format"] {
        let code = fixture_error_after_schema_mutation("request.schema.json", |schema| {
            schema[keyword] = json!("ignored_assertion");
        })?;
        assert_eq!(code, "schema_compile_failed");
    }

    type SchemaMutation = fn(&mut Value);
    let open_object_schema_cases: &[(&str, SchemaMutation)] = &[
        (
            "object_schema_missing_additional_properties",
            |schema: &mut Value| {
                schema["properties"]["auto_repair_preference"] =
                    json!({ "type": "object", "properties": { "x": { "type": "string" } } });
            },
        ),
        (
            "object_schema_allows_additional_properties",
            |schema: &mut Value| {
                schema["additionalProperties"] = json!(true);
            },
        ),
    ];
    for (case_name, mutate) in open_object_schema_cases {
        let code = fixture_error_after_schema_mutation("request.schema.json", *mutate)?;
        assert_eq!(code, "schema_compile_failed", "{case_name} should reject");
    }

    let malformed_keyword_cases: &[(&str, SchemaMutation)] = &[
        ("required_string", |schema| {
            schema["required"] = json!("title")
        }),
        ("additional_properties_string", |schema| {
            schema["additionalProperties"] = json!("false");
        }),
        ("properties_array", |schema| {
            schema["properties"] = json!([])
        }),
        ("items_string", |schema| {
            schema["properties"]["desired_artifacts"]["items"] = json!("string");
        }),
        ("min_items_string", |schema| {
            schema["properties"]["desired_artifacts"]["minItems"] = json!("1");
        }),
        ("max_items_string", |schema| {
            schema["properties"]["constraints"]["maxItems"] = json!("12");
        }),
        ("min_length_string", |schema| {
            schema["properties"]["title"]["minLength"] = json!("1");
        }),
        ("max_length_string", |schema| {
            schema["properties"]["title"]["maxLength"] = json!("120");
        }),
        ("unique_items_string", |schema| {
            schema["properties"]["desired_artifacts"]["uniqueItems"] = json!("true");
        }),
    ];
    for (_case_name, mutate) in malformed_keyword_cases {
        let code = fixture_error_after_schema_mutation("request.schema.json", *mutate)?;
        assert_eq!(code, "schema_compile_failed");
    }

    let empty_closed_object_error =
        fixture_error_after_schema_mutation("proposed_task_graph.schema.json", |schema| {
            schema["properties"]["source_request_summary"] =
                json!({ "type": "object", "additionalProperties": false });
        })?;
    assert_eq!(
        empty_closed_object_error,
        "fixture_schema_validation_failed"
    );
    Ok(())
}

fn fixture_error_after_schema_mutation<F>(
    schema_file: &str,
    mutate: F,
) -> Result<&'static str, Box<dyn Error>>
where
    F: FnOnce(&mut Value),
{
    let temp = tempfile::tempdir()?;
    copy_tree(
        &workspace_root().join("schemas"),
        &temp.path().join("schemas"),
    )?;
    copy_tree(
        &workspace_root().join("examples").join("mvp"),
        &temp.path().join("examples").join("mvp"),
    )?;
    let schema_path = temp.path().join("schemas").join(schema_file);
    let mut schema: Value = serde_json::from_str(&fs::read_to_string(&schema_path)?)?;
    mutate(&mut schema);
    fs::write(schema_path, serde_json::to_string_pretty(&schema)?)?;
    let error = match verify_fixture_set(temp.path()) {
        Ok(_) => return Err("schema mutation should reject".into()),
        Err(error) => error,
    };
    Ok(error.code)
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
        "verifier_runner_id": "actor_verifier_001",
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
        "work_packet_ids": ["wp_energy_001_generate_pack"],
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
