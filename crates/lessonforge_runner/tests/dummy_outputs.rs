use std::error::Error;

use lessonforge_runner::{
    DummyGenerationContext, DummyPlanningContext, DummyVerificationContext, RunnerMode,
    dummy_plan_verification, dummy_proposed_task_graph, dummy_request_moderation_report,
    validate_runner_config, write_dummy_artifact_bundle,
};
use lessonforge_schema::{
    validate_artifact_manifest, validate_plan_verification, validate_proposed_task_graph,
    validate_request_moderation_report,
};
use lessonforge_validator::{
    ArtifactValidationContext, CheckerExecutionMode, SubmittedArtifactDigests, ValidationCheckName,
    ValidationReportStatus, validate_bundle,
};

#[test]
fn dummy_moderator_planner_and_verifier_emit_schema_valid_safe_json() -> Result<(), Box<dyn Error>>
{
    let moderator = validate_runner_config(&dummy_config(
        "runner_dummy_moderator_001",
        "Dummy Moderator",
        RunnerMode::DummyRequestModerator,
    ))?;
    let moderation = dummy_request_moderation_report(&moderator)?;
    validate_request_moderation_report(&moderation)?;
    assert_safe_runner_output(&moderation.to_string());
    assert!(moderation.get("claim_token").is_none());

    let planner = validate_runner_config(&dummy_config(
        "actor_planner_001",
        "Dummy Planner",
        RunnerMode::DummyPlanner,
    ))?;
    let proposal = dummy_proposed_task_graph(&planner, &DummyPlanningContext::mvp_fixture())?;
    validate_proposed_task_graph(&proposal)?;
    assert_safe_runner_output(&proposal.to_string());

    let verifier = validate_runner_config(&dummy_config(
        "actor_verifier_001",
        "Dummy Verifier",
        RunnerMode::DummyPlanVerifier,
    ))?;
    let verification =
        dummy_plan_verification(&verifier, &DummyVerificationContext::mvp_fixture())?;
    validate_plan_verification(&verification)?;
    assert_safe_runner_output(&verification.to_string());

    assert_eq!(
        plan_verification_error_code(&planner),
        "runner_mode_mismatch"
    );
    Ok(())
}

#[test]
fn dummy_generator_writes_valid_bundle_and_authoritative_digests() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let generator = validate_runner_config(&dummy_config_with_workspace(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
        temp.path(),
    ))?;
    let context = DummyGenerationContext::mvp_fixture_for_workspace(temp.path());
    let output_dir = context.output_dir();
    let output = write_dummy_artifact_bundle(&generator, &context, &output_dir)?;

    assert_eq!(output.kind, "generation_output_v1");
    assert_eq!(
        output.artifact_bundle_reference.kind,
        "artifact_bundle_reference"
    );
    assert_eq!(
        output.artifact_bundle_reference.artifact_intake_ref,
        context.artifact_intake_ref
    );
    assert_eq!(output.provenance.file_digests.len(), 5);
    assert!(output.provenance.bundle_digest.starts_with("sha256:"));
    assert_eq!(
        output.provenance.file_digests,
        output.provenance.runner_self_test_report.file_digests
    );
    assert_eq!(
        output.provenance.bundle_digest,
        output.provenance.runner_self_test_report.bundle_digest
    );
    assert_eq!(
        output.provenance.runner_self_test_report.self_test_status,
        "not_run_sandbox_unavailable"
    );
    assert_eq!(
        output.provenance.runner_self_test_report.work_packet_id,
        context.work_packet_id
    );
    assert_eq!(
        output.provenance.runner_self_test_report.runner_actor_id,
        context.runner_actor_id
    );
    assert_eq!(
        output
            .provenance
            .runner_self_test_report
            .attestation
            .runner_key_id,
        "rkey_dummy_generator_001"
    );
    assert!(
        output
            .provenance
            .runner_self_test_report
            .attestation
            .signature
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    );
    assert_ne!(
        output
            .provenance
            .runner_self_test_report
            .attestation
            .signed_payload_digest,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert!(format!("{output:?}").find("claim_token").is_none());
    assert_safe_runner_output(&format!("{output:?}"));

    let other_temp = tempfile::tempdir()?;
    let other_generator = validate_runner_config(&dummy_config_with_workspace_and_key(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
        other_temp.path(),
        "rkey_dummy_generator_002",
        &[8; 32],
    ))?;
    let other_context = DummyGenerationContext::mvp_fixture_for_workspace(other_temp.path());
    let other_output = write_dummy_artifact_bundle(
        &other_generator,
        &other_context,
        &other_context.output_dir(),
    )?;
    assert_ne!(
        output
            .provenance
            .runner_self_test_report
            .attestation
            .signed_payload_digest,
        other_output
            .provenance
            .runner_self_test_report
            .attestation
            .signed_payload_digest
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output_dir.join("manifest.json"))?)?;
    assert_eq!(
        output.artifact_bundle_reference.manifest_summary.contents,
        serde_json::from_value::<Vec<String>>(manifest["contents"].clone())?
    );
    assert_eq!(
        output
            .artifact_bundle_reference
            .manifest_summary
            .known_limitations,
        serde_json::from_value::<Vec<String>>(manifest["known_limitations"].clone())?
    );
    validate_artifact_manifest(&manifest)?;

    let validation_context = ArtifactValidationContext {
        validation_report_id: "vreport_energy_001".to_owned(),
        artifact_id: context.artifact_id.clone(),
        request_id: context.request_id.clone(),
        generation_work_packet_id: context.work_packet_id.clone(),
        generation_execution_policy: context.execution_policy.clone(),
        runner_actor_id: context.runner_actor_id.clone(),
        validator_version: "lessonforge_validator_static_v1".to_owned(),
        execution_mode: CheckerExecutionMode::StaticOnly,
        submitted_digests: Some(SubmittedArtifactDigests {
            file_digests: file_digest_records(&output_dir, &output.provenance.file_digests)?,
            bundle_digest: output.provenance.bundle_digest.clone(),
        }),
    };
    let report = validate_bundle(&output_dir, &validation_context)?;
    assert_eq!(report.status, ValidationReportStatus::IncompleteStaticOnly);
    assert_eq!(
        report.check_status(ValidationCheckName::PublicProvenanceAllowlist),
        Some(lessonforge_validator::CheckStatus::Passed)
    );
    assert_eq!(
        report.authoritative_digests.bundle_digest,
        output.provenance.bundle_digest
    );
    Ok(())
}

#[test]
fn dummy_generator_requires_valid_attestation_config() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let mut missing = dummy_config(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
    );
    missing.runner.workspace_root = temp.path().display().to_string();
    assert_eq!(config_error_code(&missing), "invalid_attestation_config");

    let mut wrong_path = dummy_config_with_workspace(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
        temp.path(),
    );
    wrong_path.attestation.runner_key_id = "rkey_dummy_generator_001".to_owned();
    wrong_path.attestation.ed25519_private_key_path = tempfile::tempdir()?
        .path()
        .join("key.hex")
        .display()
        .to_string();
    assert_eq!(config_error_code(&wrong_path), "invalid_attestation_config");

    let large_key_dir = temp.path().join("large-key");
    std::fs::create_dir_all(&large_key_dir)?;
    let large_key_path = large_key_dir.join("runner.hex");
    std::fs::write(&large_key_path, vec![b'a'; 2048])?;
    let mut large_key = dummy_config_with_workspace(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
        temp.path(),
    );
    large_key.attestation.runner_key_id = "rkey_dummy_generator_001".to_owned();
    large_key.attestation.ed25519_private_key_path = large_key_path.display().to_string();
    assert_eq!(config_error_code(&large_key), "invalid_attestation_config");

    let uppercase_key_dir = temp.path().join("uppercase-key");
    std::fs::create_dir_all(&uppercase_key_dir)?;
    let uppercase_key_path = uppercase_key_dir.join("runner.hex");
    std::fs::write(
        &uppercase_key_path,
        hex_seed(&[10; 32]).to_ascii_uppercase(),
    )?;
    let mut uppercase_key = dummy_config_with_workspace(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
        temp.path(),
    );
    uppercase_key.attestation.runner_key_id = "rkey_dummy_generator_001".to_owned();
    uppercase_key.attestation.ed25519_private_key_path = uppercase_key_path.display().to_string();
    validate_runner_config(&uppercase_key)?;

    #[cfg(unix)]
    {
        let outside = tempfile::tempdir()?;
        let outside_key = outside.path().join("runner.hex");
        std::fs::write(&outside_key, hex_seed(&[8; 32]))?;
        let symlink_key_path = temp.path().join("keys").join("symlink-runner.hex");
        let Some(key_parent) = symlink_key_path.parent() else {
            return Err("symlink key path should have parent".into());
        };
        std::fs::create_dir_all(key_parent)?;
        std::os::unix::fs::symlink(&outside_key, &symlink_key_path)?;
        let mut symlink_key = dummy_config_with_workspace(
            "actor_generator_001",
            "Dummy Generator",
            RunnerMode::DummyGenerator,
            temp.path(),
        );
        symlink_key.attestation.runner_key_id = "rkey_dummy_generator_001".to_owned();
        symlink_key.attestation.ed25519_private_key_path = symlink_key_path.display().to_string();
        assert_eq!(
            config_error_code(&symlink_key),
            "invalid_attestation_config"
        );

        let socket_key_path = temp.path().join("keys").join("socket-runner.hex");
        let _socket = std::os::unix::net::UnixListener::bind(&socket_key_path)?;
        let mut socket_key = dummy_config_with_workspace(
            "actor_generator_001",
            "Dummy Generator",
            RunnerMode::DummyGenerator,
            temp.path(),
        );
        socket_key.attestation.runner_key_id = "rkey_dummy_generator_001".to_owned();
        socket_key.attestation.ed25519_private_key_path = socket_key_path.display().to_string();
        assert_eq!(config_error_code(&socket_key), "invalid_attestation_config");
    }

    let mut planner_with_key = dummy_config(
        "actor_planner_001",
        "Dummy Planner",
        RunnerMode::DummyPlanner,
    );
    planner_with_key.attestation.runner_key_id = "rkey_dummy_generator_001".to_owned();
    planner_with_key.attestation.ed25519_private_key_path = temp
        .path()
        .join("keys")
        .join("runner.hex")
        .display()
        .to_string();
    assert_eq!(
        config_error_code(&planner_with_key),
        "invalid_attestation_config"
    );
    Ok(())
}

#[test]
fn dummy_generator_rejects_wrong_mode_and_path_traversal() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let planner = validate_runner_config(&dummy_config_with_workspace(
        "actor_planner_001",
        "Dummy Planner",
        RunnerMode::DummyPlanner,
        temp.path(),
    ))?;
    let context = DummyGenerationContext::mvp_fixture_for_workspace(temp.path());
    assert_eq!(
        artifact_bundle_error_code(&planner, &context, &context.output_dir()),
        "runner_mode_mismatch"
    );

    let generator = validate_runner_config(&dummy_config_with_workspace(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
        temp.path(),
    ))?;
    assert_eq!(
        artifact_bundle_error_code(&generator, &context, std::path::Path::new("../outside")),
        "unsafe_workspace_path"
    );
    assert_eq!(
        artifact_bundle_error_code(&generator, &context, temp.path()),
        "unsafe_workspace_path"
    );
    let mut mismatched_context = DummyGenerationContext::mvp_fixture_for_workspace(temp.path());
    mismatched_context.claim_workspace_root = temp
        .path()
        .join("other")
        .join("claims")
        .join(&context.lease_id);
    assert_eq!(
        artifact_bundle_error_code(&generator, &mismatched_context, &context.output_dir()),
        "unsafe_workspace_path"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn dummy_generator_rejects_symlinked_claim_output_dir() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let generator = validate_runner_config(&dummy_config_with_workspace(
        "actor_generator_001",
        "Dummy Generator",
        RunnerMode::DummyGenerator,
        temp.path(),
    ))?;
    let context = DummyGenerationContext::mvp_fixture_for_workspace(temp.path());
    std::fs::create_dir_all(&context.claim_workspace_root)?;
    std::os::unix::fs::symlink(outside.path(), context.output_dir())?;
    assert_eq!(
        artifact_bundle_error_code(&generator, &context, &context.output_dir()),
        "unsafe_workspace_path"
    );
    Ok(())
}

fn plan_verification_error_code(
    validated: &lessonforge_runner::ValidatedRunnerConfig,
) -> &'static str {
    match dummy_plan_verification(validated, &DummyVerificationContext::mvp_fixture()) {
        Ok(_) => "unexpected_ok",
        Err(error) => error.safe_code(),
    }
}

fn artifact_bundle_error_code(
    validated: &lessonforge_runner::ValidatedRunnerConfig,
    context: &DummyGenerationContext,
    output_root: &std::path::Path,
) -> &'static str {
    match write_dummy_artifact_bundle(validated, context, output_root) {
        Ok(_) => "unexpected_ok",
        Err(error) => error.safe_code(),
    }
}

fn file_digest_records(
    output_dir: &std::path::Path,
    digests: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<lessonforge_validator::FileDigestRecord>, Box<dyn Error>> {
    let mut records = Vec::new();
    for path in [
        "answer_key.md",
        "checker.py",
        "manifest.json",
        "teacher_notes.md",
        "worksheet.md",
    ] {
        records.push(lessonforge_validator::FileDigestRecord {
            path: path.to_owned(),
            sha256: digests
                .get(path)
                .ok_or_else(|| format!("missing digest for {path}"))?
                .clone(),
            size_bytes: std::fs::metadata(output_dir.join(path))?.len(),
        });
    }
    Ok(records)
}

fn dummy_config(
    runner_id: &str,
    public_name: &str,
    mode: RunnerMode,
) -> lessonforge_runner::RunnerConfig {
    lessonforge_runner::RunnerConfig::dummy(runner_id, public_name, mode, "https://127.0.0.1:8443")
}

fn dummy_config_with_workspace(
    runner_id: &str,
    public_name: &str,
    mode: RunnerMode,
    workspace_root: &std::path::Path,
) -> lessonforge_runner::RunnerConfig {
    dummy_config_with_workspace_and_key(
        runner_id,
        public_name,
        mode,
        workspace_root,
        "rkey_dummy_generator_001",
        &[7; 32],
    )
}

fn dummy_config_with_workspace_and_key(
    runner_id: &str,
    public_name: &str,
    mode: RunnerMode,
    workspace_root: &std::path::Path,
    runner_key_id: &str,
    seed: &[u8; 32],
) -> lessonforge_runner::RunnerConfig {
    let mut config = dummy_config(runner_id, public_name, mode);
    config.runner.workspace_root = workspace_root.display().to_string();
    if mode == RunnerMode::DummyGenerator {
        let key_dir = workspace_root.join("keys");
        let key_path = key_dir.join("dummy-runner-ed25519.hex");
        std::fs::create_dir_all(&key_dir).ok();
        std::fs::write(&key_path, hex_seed(seed)).ok();
        config.attestation.runner_key_id = runner_key_id.to_owned();
        config.attestation.ed25519_private_key_path = key_path.display().to_string();
    }
    config
}

fn config_error_code(config: &lessonforge_runner::RunnerConfig) -> &'static str {
    match validate_runner_config(config) {
        Ok(_) => "unexpected_ok",
        Err(error) => error.safe_code(),
    }
}

fn hex_seed(seed: &[u8; 32]) -> String {
    seed.iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn assert_safe_runner_output(output: &str) {
    for forbidden in [
        "provider", "prompt:", "prompt=", "api_key", "sk-", "/Users/", "/tmp/", "http://",
        "https://",
    ] {
        assert!(
            !output.to_ascii_lowercase().contains(forbidden),
            "output leaked forbidden marker: {forbidden}"
        );
    }
}
