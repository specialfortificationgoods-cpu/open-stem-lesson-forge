use lessonforge_runner::{
    DummyGenerationContext, DummyPlanningContext, DummyVerificationContext, RunnerConfig,
    RunnerMode, capability_summary, dummy_plan_verification, dummy_proposed_task_graph,
    dummy_request_moderation_report, validate_runner_config, write_dummy_artifact_bundle,
};
use lessonforge_validator::{
    ArtifactValidationContext, CheckerExecutionMode, ValidationReportStatus, validate_bundle,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_ROWS: &[(&str, &str)] = &[
    ("E2E-001", "automated"),
    ("E2E-002", "automated"),
    ("REQ-GATE-001", "automated"),
    ("REQ-GATE-002", "automated"),
    ("REQ-GATE-003", "automated"),
    ("REQ-GATE-004", "automated"),
    ("REQ-GATE-005", "automated"),
    ("GRAPH-GATE-001", "automated"),
    ("GRAPH-GATE-002", "automated"),
    ("GRAPH-GATE-003", "automated"),
    ("GRAPH-GATE-004", "automated"),
    ("GRAPH-GATE-005", "automated"),
    ("LEASE-001", "automated"),
    ("LEASE-002", "automated"),
    ("LEASE-003", "automated"),
    ("LEASE-004", "automated"),
    ("STATE-GATE-001", "automated"),
    ("RUNNER-AUTH-001", "automated"),
    ("DB-GATE-001", "automated"),
    ("NINF-001", "automated"),
    ("NINF-002", "automated"),
    ("NINF-003", "automated"),
    ("NINF-004", "automated"),
    ("LEAK-001", "automated"),
    ("LEAK-002", "automated"),
    ("LEAK-003", "automated"),
    ("LEAK-004", "deferred_by_spec"),
    ("RUN-GATE-001", "automated"),
    ("RUN-GATE-002", "automated"),
    ("RUN-GATE-003", "automated"),
    ("RUN-GATE-004", "automated"),
    ("CODE-GATE-001", "automated"),
    ("CODE-GATE-002", "automated"),
    ("CODE-GATE-003", "automated"),
    ("CODE-GATE-004", "automated"),
    ("ART-GATE-001", "automated"),
    ("ART-GATE-002", "automated"),
    ("ART-GATE-003", "automated"),
    ("REVIEW-GATE-001", "automated"),
    ("REVIEW-GATE-002", "automated"),
    ("REVIEW-GATE-003", "automated"),
    ("REVIEW-GATE-004", "automated"),
    ("REVIEW-GATE-005", "automated"),
    ("CR-GATE-001", "deferred_by_spec"),
    ("CR-GATE-002", "deferred_by_spec"),
    ("CR-GATE-003", "deferred_by_spec"),
    ("CR-GATE-004", "deferred_by_spec"),
    ("CR-GATE-005", "deferred_by_spec"),
    ("CR-GATE-006", "deferred_by_spec"),
    ("CR-GATE-007", "deferred_by_spec"),
    ("API-GATE-001", "automated"),
    ("API-GATE-002", "automated"),
];

const DEFERRED_NEGATIVE_ROWS: &[&str] = &[
    "LEAK-004",
    "API-GATE-002",
    "CR-GATE-001",
    "CR-GATE-002",
    "CR-GATE-003",
    "CR-GATE-004",
    "CR-GATE-005",
    "CR-GATE-006",
    "CR-GATE-007",
];

#[derive(Debug, Deserialize)]
struct Manifest {
    spec: String,
    suite: String,
    rows: Vec<ManifestRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestRow {
    id: String,
    status: String,
    command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    suite: String,
    case_id: Option<String>,
    root: PathBuf,
}

fn main() {
    match run() {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<String, String> {
    let args = parse_args(env::args().skip(1).collect())?;
    if args.suite != "mvp" {
        return Err("unsupported_suite".to_owned());
    }
    let manifest = load_manifest(&args.root)?;
    let rows = validate_manifest(&manifest)?;

    if let Some(case_id) = args.case_id {
        let Some(row) = rows.get(&case_id) else {
            return Err("unknown_case".to_owned());
        };
        let case_execution = run_case(&args.root, row)?;
        let suffix = if is_deferred_negative_case(row.id.as_str()) {
            " negative_unavailable_verified"
        } else {
            ""
        };
        return Ok(format!(
            "{} {} {} via {}{}",
            row.id,
            row.status,
            case_execution.label(),
            row.command,
            suffix
        ));
    }

    // One deterministic smoke run covers the full-slice E2E rows.
    run_full_mvp_smoke(&args.root)?;
    for &row_id in DEFERRED_NEGATIVE_ROWS {
        let row = rows
            .get(row_id)
            .ok_or_else(|| "manifest_missing_required_row".to_owned())?;
        run_case(&args.root, row)?;
    }
    let mut output = String::from("mvp suite passed\n");
    for (id, _) in REQUIRED_ROWS {
        output.push_str(id);
        if is_deferred_negative_case(id) {
            output.push_str(" executed/passed negative_unavailable_verified");
        } else if matches!(*id, "E2E-001" | "E2E-002") {
            output.push_str(" executed_shared_smoke");
        } else {
            output.push_str(" listed_in_manifest");
        }
        output.push('\n');
    }
    Ok(output)
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut suite: Option<String> = None;
    let mut case_id: Option<String> = None;
    let mut root = env::current_dir().map_err(|error| error.to_string())?;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--suite" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing_suite".to_owned());
                };
                suite = Some(value.clone());
                index += 2;
            }
            "--case" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing_case".to_owned());
                };
                case_id = Some(value.clone());
                index += 2;
            }
            "--root" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing_root".to_owned());
                };
                root = PathBuf::from(value);
                index += 2;
            }
            _ => return Err("unknown_argument".to_owned()),
        }
    }
    Ok(Args {
        suite: suite.ok_or_else(|| "missing_suite".to_owned())?,
        case_id,
        root,
    })
}

fn load_manifest(root: &Path) -> Result<Manifest, String> {
    let path = root
        .join("docs")
        .join("implementation")
        .join("mvp-test-manifest.json");
    let text = fs::read_to_string(&path).map_err(|_| "manifest_unavailable".to_owned())?;
    serde_json::from_str(&text).map_err(|_| "manifest_invalid_json".to_owned())
}

fn validate_manifest(manifest: &Manifest) -> Result<BTreeMap<String, ManifestRow>, String> {
    if manifest.spec != "012" || manifest.suite != "mvp" {
        return Err("manifest_identity_mismatch".to_owned());
    }
    let mut rows = BTreeMap::new();
    for row in &manifest.rows {
        if row.command.trim().is_empty() {
            return Err("manifest_row_command_missing".to_owned());
        }
        if rows.insert(row.id.clone(), row.clone()).is_some() {
            return Err("manifest_duplicate_row".to_owned());
        }
    }
    for (id, status) in REQUIRED_ROWS {
        let Some(row) = rows.get(*id) else {
            return Err("manifest_missing_required_row".to_owned());
        };
        if row.status != *status {
            return Err("manifest_required_status_mismatch".to_owned());
        }
    }
    let required = REQUIRED_ROWS
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect::<BTreeSet<_>>();
    if rows.keys().any(|id| !required.contains(id)) {
        return Err("manifest_unknown_row".to_owned());
    }
    Ok(rows)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseExecution {
    Executed,
    ListedDelegated,
}

impl CaseExecution {
    fn label(self) -> &'static str {
        match self {
            Self::Executed => "executed/passed",
            Self::ListedDelegated => "listed/delegated",
        }
    }
}

fn run_case(root: &Path, row: &ManifestRow) -> Result<CaseExecution, String> {
    match row.id.as_str() {
        "E2E-001" | "E2E-002" => run_full_mvp_smoke(root).map(|()| CaseExecution::Executed),
        "LEAK-004" => verify_unavailable_transport_surface(root, &["/v1/events/stream"])
            .map(|()| CaseExecution::Executed),
        "API-GATE-002" => verify_unavailable_transport_surface(
            root,
            &[
                "websocket_command",
                "web_socket_command",
                "mutating_websocket",
                "/v1/ws",
                "/ws/command",
            ],
        )
        .map(|()| CaseExecution::Executed),
        "CR-GATE-001" | "CR-GATE-002" | "CR-GATE-003" | "CR-GATE-004" | "CR-GATE-005"
        | "CR-GATE-006" | "CR-GATE-007" => {
            verify_unavailable_code_repair_ingestion_surface(root).map(|()| CaseExecution::Executed)
        }
        _ => Ok(CaseExecution::ListedDelegated),
    }
}

fn is_deferred_negative_case(row_id: &str) -> bool {
    DEFERRED_NEGATIVE_ROWS.contains(&row_id)
}

fn verify_unavailable_code_repair_ingestion_surface(root: &Path) -> Result<(), String> {
    let scan_root = root.join("crates").join("lessonforge_api").join("src");
    scan_source_tree_for_absence(
        &scan_root,
        &[
            "claim_code_critique",
            "submit_code_critique",
            "claim_code_repair",
            "submit_code_repair",
            "code_critique_endpoint",
            "code_repair_endpoint",
            "/v1/code-critique",
            "/v1/code-repair",
            "/v1/code_critique",
            "/v1/code_repair",
        ],
    )
    .map_err(|error| {
        match error.as_str() {
            "unavailable_transport_surface_present" => "unavailable_code_repair_surface_present",
            _ => "negative_surface_scan_failed",
        }
        .to_owned()
    })
}

fn verify_unavailable_transport_surface(
    root: &Path,
    forbidden_markers: &[&str],
) -> Result<(), String> {
    let scan_roots = [
        root.join("crates").join("lessonforge_api").join("src"),
        root.join("crates").join("lessonforge_core").join("src"),
    ];
    for scan_root in scan_roots {
        scan_source_tree_for_absence(&scan_root, forbidden_markers)?;
    }
    Ok(())
}

fn scan_source_tree_for_absence(path: &Path, forbidden_markers: &[&str]) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "negative_surface_scan_failed")?;
    if metadata.file_type().is_symlink() {
        return Err("negative_surface_scan_failed".to_owned());
    }
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .map_err(|_| "negative_surface_scan_failed")?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "negative_surface_scan_failed")?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            scan_source_tree_for_absence(&entry.path(), forbidden_markers)?;
        }
        return Ok(());
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return Ok(());
    }
    let text = fs::read_to_string(path).map_err(|_| "negative_surface_scan_failed")?;
    let lower = text.to_ascii_lowercase();
    if forbidden_markers
        .iter()
        .any(|marker| lower.contains(&marker.to_ascii_lowercase()))
    {
        return Err("unavailable_transport_surface_present".to_owned());
    }
    Ok(())
}

fn run_full_mvp_smoke(root: &Path) -> Result<(), String> {
    lessonforge_schema::verify_fixture_set(root).map_err(|_| "schema_fixture_gate_failed")?;

    let moderator = validated_dummy(
        "runner_dummy_moderator_001",
        "Dummy moderator",
        RunnerMode::DummyRequestModerator,
    )?;
    let moderation = dummy_request_moderation_report(&moderator)
        .map_err(|_| "dummy_moderation_output_failed")?;
    lessonforge_schema::validate_request_moderation_report(&moderation)
        .map_err(|_| "dummy_moderation_schema_failed")?;

    let planner = validated_dummy(
        "actor_planner_001",
        "Dummy planner",
        RunnerMode::DummyPlanner,
    )?;
    let planner_summary =
        capability_summary(&planner).map_err(|_| "dummy_planner_summary_failed")?;
    let graph = dummy_proposed_task_graph(&planner, &DummyPlanningContext::mvp_fixture())
        .map_err(|_| "dummy_graph_output_failed")?;
    if graph
        .get("planner_runner_id")
        .and_then(|value| value.as_str())
        != Some(planner_summary.runner_id.as_str())
    {
        return Err("dummy_graph_runner_identity_mismatch".to_owned());
    }
    lessonforge_schema::validate_proposed_task_graph(&graph)
        .map_err(|_| "dummy_graph_schema_failed")?;

    let verifier = validated_dummy(
        "actor_verifier_001",
        "Dummy verifier",
        RunnerMode::DummyPlanVerifier,
    )?;
    let verifier_summary =
        capability_summary(&verifier).map_err(|_| "dummy_verifier_summary_failed")?;
    let verification = dummy_plan_verification(&verifier, &DummyVerificationContext::mvp_fixture())
        .map_err(|_| "dummy_verification_output_failed")?;
    if verification
        .get("verifier_runner_id")
        .and_then(|value| value.as_str())
        != Some(verifier_summary.runner_id.as_str())
    {
        return Err("dummy_verification_runner_identity_mismatch".to_owned());
    }
    lessonforge_schema::validate_plan_verification(&verification)
        .map_err(|_| "dummy_verification_schema_failed")?;

    let temp = tempfile::tempdir().map_err(|_| "temporary_workspace_unavailable")?;
    let workspace = temp.path().join("runner-work");
    let mut generator_config = RunnerConfig::dummy(
        "actor_generator_001",
        "Dummy generator",
        RunnerMode::DummyGenerator,
        "http://127.0.0.1:8080",
    );
    generator_config.runner.workspace_root = workspace.to_string_lossy().to_string();
    let key_dir = workspace.join("keys");
    fs::create_dir_all(&key_dir).map_err(|_| "generator_key_unavailable")?;
    let key_path = key_dir.join("dummy-runner-ed25519.hex");
    fs::write(
        &key_path,
        "0707070707070707070707070707070707070707070707070707070707070707",
    )
    .map_err(|_| "generator_key_unavailable")?;
    #[cfg(unix)]
    fs::set_permissions(
        &key_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .map_err(|_| "generator_key_unavailable")?;
    generator_config.attestation.runner_key_id = "rkey_dummy_generator_001".to_owned();
    generator_config.attestation.ed25519_private_key_path = key_path.to_string_lossy().to_string();
    let generator =
        validate_runner_config(&generator_config).map_err(|_| "generator_config_invalid")?;
    let generation_context = DummyGenerationContext::mvp_fixture_for_workspace(&workspace);
    let generation = write_dummy_artifact_bundle(
        &generator,
        &generation_context,
        &generation_context.output_dir(),
    )
    .map_err(|_| "dummy_generation_output_failed")?;
    if generation.kind != "generation_output_v1" {
        return Err("dummy_generation_kind_mismatch".to_owned());
    }

    let report = validate_bundle(
        &generation_context.output_dir(),
        &ArtifactValidationContext {
            validation_report_id: "vreport_energy_001".to_owned(),
            artifact_id: generation_context.artifact_id,
            request_id: generation_context.request_id,
            generation_work_packet_id: generation_context.work_packet_id,
            generation_execution_policy: generation_context.execution_policy,
            runner_actor_id: generation_context.runner_actor_id,
            validator_version: "mvp-e2e".to_owned(),
            execution_mode: CheckerExecutionMode::SandboxedSubprocess,
            submitted_digests: None,
        },
    )
    .map_err(|_| "artifact_validation_failed")?;
    if !report.opens_machine_validation() || report.status != ValidationReportStatus::Passed {
        return Err("artifact_validation_not_machine_validated".to_owned());
    }
    Ok(())
}

fn validated_dummy(
    runner_id: &str,
    public_name: &str,
    mode: RunnerMode,
) -> Result<lessonforge_runner::ValidatedRunnerConfig, String> {
    let config = RunnerConfig::dummy(runner_id, public_name, mode, "http://127.0.0.1:8080");
    validate_runner_config(&config).map_err(|_| "runner_config_invalid".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn source_tree_scan_rejects_symlinked_entries() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let source_root = temp.path().join("src");
        fs::create_dir(&source_root)?;
        fs::write(outside.path().join("hidden.rs"), "websocket_command")?;
        std::os::unix::fs::symlink(
            outside.path().join("hidden.rs"),
            source_root.join("hidden.rs"),
        )?;

        let error = scan_source_tree_for_absence(&source_root, &["websocket_command"]);

        assert_eq!(error, Err("negative_surface_scan_failed".to_owned()));
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn source_tree_scan_rejects_missing_and_dangling_symlink_paths_safely()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let source_root = temp.path().join("src");
        fs::create_dir(&source_root)?;
        let missing = temp.path().join("missing").join("secret.rs");
        let dangling = source_root.join("dangling.rs");
        std::os::unix::fs::symlink(&missing, &dangling)?;

        assert_eq!(
            scan_source_tree_for_absence(&dangling, &["websocket_command"]),
            Err("negative_surface_scan_failed".to_owned())
        );
        assert_eq!(
            scan_source_tree_for_absence(&missing, &["websocket_command"]),
            Err("negative_surface_scan_failed".to_owned())
        );
        Ok(())
    }

    #[test]
    fn manifest_load_errors_do_not_echo_root_paths_or_os_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let unsafe_root = temp.path().join("sk-proj-redaction-test");
        let Err(error) = load_manifest(&unsafe_root) else {
            return Err("missing manifest should fail".into());
        };

        assert_eq!(error, "manifest_unavailable");
        let rendered_root = unsafe_root.display().to_string();
        assert!(!error.contains(&rendered_root));
        assert!(!error.contains("sk-"));
        assert!(!error.contains("/private/tmp"));
        assert!(!error.contains("No such file"));
        Ok(())
    }

    #[test]
    fn manifest_rejects_whitespace_only_commands() {
        let manifest = Manifest {
            spec: "012".to_owned(),
            suite: "mvp".to_owned(),
            rows: vec![ManifestRow {
                id: "E2E-001".to_owned(),
                status: "automated".to_owned(),
                command: " \t\n".to_owned(),
            }],
        };

        assert_eq!(
            validate_manifest(&manifest).err().as_deref(),
            Some("manifest_row_command_missing")
        );
    }
}
