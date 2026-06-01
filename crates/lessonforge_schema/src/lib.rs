use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

const REQUEST_SCHEMA: &str = "request.schema.json";
const REQUEST_MODERATION_REPORT_SCHEMA: &str = "request_moderation_report.schema.json";
const PROPOSED_TASK_GRAPH_SCHEMA: &str = "proposed_task_graph.schema.json";
const PLAN_VERIFICATION_SCHEMA: &str = "plan_verification.schema.json";
const ARTIFACT_MANIFEST_SCHEMA: &str = "artifact_manifest.schema.json";
const PLAN_VERIFICATION_SAFE_FINDING_MESSAGE_PATTERN: &str = "^(?!.*(?:://|@|[Ss][Ee][Cc][Rr][Ee][Tt]|[Aa][Pp][Ii]_[Kk][Ee][Yy]|[Tt][Oo][Kk][Ee][Nn]|[Cc][Oo][Oo][Kk][Ii][Ee]|[Cc][Rr][Ee][Dd][Ee][Nn][Tt][Ii][Aa][Ll]|[Pp][Aa][Ss][Ss][Ww][Oo][Rr][Dd]|[Ss][Tt][Uu][Dd][Ee][Nn][Tt] [Rr][Ee][Cc][Oo][Rr][Dd]|[Ss][Tt][Uu][Dd][Ee][Nn][Tt] [Gg][Rr][Aa][Dd][Ee]|[Ss][Tt][Uu][Dd][Ee][Nn][Tt] [Pp][Ll][Aa][Cc][Ee][Mm][Ee][Nn][Tt]|[Ss][Tt][Uu][Dd][Ee][Nn][Tt] [Pp][Rr][Oo][Ff][Ii][Ll][Ee]|[Pp][Ll][Aa][Cc][Ee][Mm][Ee][Nn][Tt] [Dd][Ee][Cc][Ii][Ss][Ii][Oo][Nn]|[Dd][Ii][Ss][Cc][Ii][Pp][Ll][Ii][Nn][Aa][Rr][Yy] [Rr][Ee][Cc][Oo][Rr][Dd]|[Dd][Ii][Ss][Cc][Ii][Pp][Ll][Ii][Nn][Aa][Rr][Yy] [Aa][Cc][Tt][Ii][Oo][Nn]|/Users/|/home/|/etc/|/private/|/var/|/tmp/|C:\\\\|\\.\\./|~/|[Ss][Ss][Hh]/|[Hh][Uu][Mm][Aa][Nn] [Aa][Pp][Pp][Rr][Oo][Vv][Aa][Ll]|[Hh][Uu][Mm][Aa][Nn] [Aa][Pp][Pp][Rr][Oo][Vv][Ee][Dd]|[Pp][Ee][Ee][Rr] [Rr][Ee][Vv][Ii][Ee][Ww][Ee][Dd]|[Pp][Ee][Ee][Rr]_[Rr][Ee][Vv][Ii][Ee][Ww][Ee][Dd]|[Pp][Rr][Oo][Mm][Oo][Tt][Ee]|[Pp][Rr][Oo][Mm][Oo][Tt][Ii][Oo][Nn]|[Pp][Uu][Bb][Ll][Ii][Ss][Hh]|[Pp][Uu][Bb][Ll][Ii][Cc][Aa][Tt][Ii][Oo][Nn]|[Ss][Tt][Aa][Tt][Ee] [Oo][Vv][Ee][Rr][Rr][Ii][Dd][Ee]|[Oo][Vv][Ee][Rr][Rr][Ii][Dd][Ee] [Ss][Tt][Aa][Tt][Ee]|[Aa][Cc][Cc][Ee][Pp][Tt][Ee][Dd] [Vv][Ee][Rr][Ii][Ff][Ii][Cc][Aa][Tt][Ii][Oo][Nn])).{1,500}$";

// Spec 010 keeps python_checker_runs in the proposed validation plan; static
// validation reports it as skipped_static_only, while sandboxed mode must run it.
const VALIDATION_CHECKS: &[&str] = &[
    "manifest_schema",
    "required_files",
    "license_metadata",
    "ai_assistance_disclosure",
    "obvious_pii_heuristic",
    "obvious_inappropriate_content_heuristic",
    "python_checker_runs",
    "no_external_network_static",
];

const DESIRED_ARTIFACTS: &[&str] = &["worksheet", "answer_key", "python_checker", "teacher_notes"];
const MANIFEST_CONTENTS: &[&str] = &[
    "manifest.json",
    "worksheet.md",
    "answer_key.md",
    "checker.py",
    "teacher_notes.md",
];

pub fn crate_boundary() -> &'static str {
    "schema_contracts"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaName {
    MvpRequest,
    RequestModerationReport,
    ProposedTaskGraph,
    PlanVerification,
    ArtifactManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureVerificationReport {
    pub checked_fixture_count: usize,
    pub checked_schemas: Vec<SchemaName>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SchemaError {
    pub schema: SchemaName,
    pub code: &'static str,
    pub field_path: String,
}

impl SchemaError {
    fn new(schema: SchemaName, code: &'static str, field_path: impl Into<String>) -> Self {
        Self {
            schema,
            code,
            field_path: field_path.into(),
        }
    }
}

impl fmt::Debug for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaError")
            .field("schema", &self.schema)
            .field("code", &self.code)
            .field("field_path", &self.field_path)
            .finish()
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?}: {} at {}",
            self.schema, self.code, self.field_path
        )
    }
}

impl std::error::Error for SchemaError {}

pub fn verify_fixture_set(
    root: impl AsRef<Path>,
) -> Result<FixtureVerificationReport, SchemaError> {
    let root = root.as_ref();
    require_exact_fixture_files(root)?;
    require_schema_files(root)?;
    let examples = root.join("examples").join("mvp");

    let request = read_json(&examples.join("request.valid.json"), SchemaName::MvpRequest)?;
    validate_against_schema(root, REQUEST_SCHEMA, SchemaName::MvpRequest, &request)?;
    validate_mvp_request(&request)?;

    let moderation = read_json(
        &examples.join("request_moderation_report.valid.json"),
        SchemaName::RequestModerationReport,
    )?;
    validate_against_schema(
        root,
        REQUEST_MODERATION_REPORT_SCHEMA,
        SchemaName::RequestModerationReport,
        &moderation,
    )?;
    validate_request_moderation_report(&moderation)?;

    let graph = read_json(
        &examples.join("proposed_task_graph.valid.json"),
        SchemaName::ProposedTaskGraph,
    )?;
    validate_against_schema(
        root,
        PROPOSED_TASK_GRAPH_SCHEMA,
        SchemaName::ProposedTaskGraph,
        &graph,
    )?;
    validate_proposed_task_graph(&graph)?;

    let verification = read_json(
        &examples.join("plan_verification.valid.json"),
        SchemaName::PlanVerification,
    )?;
    validate_against_schema(
        root,
        PLAN_VERIFICATION_SCHEMA,
        SchemaName::PlanVerification,
        &verification,
    )?;
    validate_plan_verification(&verification)?;

    let manifest = read_json(
        &examples.join("artifact_manifest.valid.json"),
        SchemaName::ArtifactManifest,
    )?;
    validate_against_schema(
        root,
        ARTIFACT_MANIFEST_SCHEMA,
        SchemaName::ArtifactManifest,
        &manifest,
    )?;
    validate_artifact_manifest(&manifest)?;

    Ok(FixtureVerificationReport {
        checked_fixture_count: 5,
        checked_schemas: vec![
            SchemaName::MvpRequest,
            SchemaName::RequestModerationReport,
            SchemaName::ProposedTaskGraph,
            SchemaName::PlanVerification,
            SchemaName::ArtifactManifest,
        ],
    })
}

pub fn validate_mvp_request(value: &Value) -> Result<(), SchemaError> {
    if let Value::Object(object) = value
        && let Some(auto_repair_preference) = object.get("auto_repair_preference")
        && !auto_repair_preference.is_string()
    {
        return Err(SchemaError::new(
            SchemaName::MvpRequest,
            "invalid_shape",
            "/auto_repair_preference",
        ));
    }
    let request: MvpRequest = deserialize(SchemaName::MvpRequest, value)?;
    require_eq(
        SchemaName::MvpRequest,
        request.subject == "physics",
        "invalid_subject",
        "/subject",
    )?;
    require_eq(
        SchemaName::MvpRequest,
        request.topic == "conservation_of_energy",
        "invalid_topic",
        "/topic",
    )?;
    require_eq(
        SchemaName::MvpRequest,
        request.age_range == "14-16",
        "invalid_age_range",
        "/age_range",
    )?;
    require_eq(
        SchemaName::MvpRequest,
        request.language == "en",
        "invalid_language",
        "/language",
    )?;
    require_eq(
        SchemaName::MvpRequest,
        request.lesson_duration_minutes == 45,
        "invalid_duration",
        "/lesson_duration_minutes",
    )?;
    require_allowed_subset(
        SchemaName::MvpRequest,
        &request.desired_artifacts,
        DESIRED_ARTIFACTS,
        "invalid_desired_artifacts",
        "/desired_artifacts",
    )?;
    require_eq(
        SchemaName::MvpRequest,
        request.license_preference == "CC-BY-4.0",
        "invalid_license",
        "/license_preference",
    )?;
    require_eq(
        SchemaName::MvpRequest,
        matches!(request.visibility.as_str(), "public" | "private"),
        "invalid_visibility",
        "/visibility",
    )?;
    require_eq(
        SchemaName::MvpRequest,
        request.forbidden_content_acknowledged,
        "missing_content_acknowledgement",
        "/forbidden_content_acknowledged",
    )?;
    validate_safe_text(SchemaName::MvpRequest, &request.title, "/title")?;
    require_eq(
        SchemaName::MvpRequest,
        request.title.chars().count() <= 120,
        "text_too_long",
        "/title",
    )?;
    validate_safe_text_list(
        SchemaName::MvpRequest,
        &request.constraints,
        "/constraints",
        12,
    )?;
    if let Some(auto_repair_preference) = &request.auto_repair_preference {
        require_eq(
            SchemaName::MvpRequest,
            auto_repair_preference == "no_automated_repair"
                || auto_repair_preference == "request_bounded_code_repair",
            "unsupported_mvp_value",
            "/auto_repair_preference",
        )?;
    }
    Ok(())
}

pub fn validate_request_moderation_report(value: &Value) -> Result<(), SchemaError> {
    let report: RequestModerationReport = deserialize(SchemaName::RequestModerationReport, value)?;
    require_prefix(
        SchemaName::RequestModerationReport,
        &report.request_moderation_report_id,
        "rmreport_",
        "/request_moderation_report_id",
    )?;
    require_prefix(
        SchemaName::RequestModerationReport,
        &report.request_moderation_task_id,
        "rmtask_",
        "/request_moderation_task_id",
    )?;
    require_prefix(
        SchemaName::RequestModerationReport,
        &report.request_id,
        "req_",
        "/request_id",
    )?;
    require_prefix(
        SchemaName::RequestModerationReport,
        &report.lease_id,
        "lease_",
        "/lease_id",
    )?;
    require_eq(
        SchemaName::RequestModerationReport,
        report.moderation_kind == "dummy_fixture",
        "invalid_moderation_kind",
        "/moderation_kind",
    )?;
    require_eq(
        SchemaName::RequestModerationReport,
        report.decision == "allow_mvp_planning",
        "invalid_moderation_decision",
        "/decision",
    )?;
    require_exact_set(
        SchemaName::RequestModerationReport,
        &report.category_flags,
        &["none"],
        "invalid_category_flags",
        "/category_flags",
    )?;
    require_exact_set(
        SchemaName::RequestModerationReport,
        &report.safe_reason_codes,
        &["moderation_allowed"],
        "invalid_safe_reason_codes",
        "/safe_reason_codes",
    )?;
    Ok(())
}

pub fn validate_proposed_task_graph(value: &Value) -> Result<(), SchemaError> {
    let graph: ProposedTaskGraph = deserialize(SchemaName::ProposedTaskGraph, value)?;
    require_prefix(
        SchemaName::ProposedTaskGraph,
        &graph.proposal_id,
        "plan_",
        "/proposal_id",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        graph.request_id == "req_energy_001",
        "proposal_request_lineage_mismatch",
        "/request_id",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        graph.planning_task_id == "ptask_energy_001",
        "proposal_planning_lineage_mismatch",
        "/planning_task_id",
    )?;
    require_prefix(
        SchemaName::ProposedTaskGraph,
        &graph.planner_runner_id,
        "actor_",
        "/planner_runner_id",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        graph.schema_version == "1.0",
        "unsupported_schema_version",
        "/schema_version",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        graph.status == "proposed",
        "invalid_status",
        "/status",
    )?;
    validate_request_summary(&graph.source_request_summary)?;
    validate_safe_text_list(
        SchemaName::ProposedTaskGraph,
        &graph.assumptions,
        "/assumptions",
        12,
    )?;
    validate_safe_text_list(
        SchemaName::ProposedTaskGraph,
        &graph.missing_information,
        "/missing_information",
        12,
    )?;
    validate_proposed_artifacts(&graph.proposed_artifacts)?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &graph.validation_plan,
        VALIDATION_CHECKS,
        "invalid_validation_plan",
        "/validation_plan",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &graph.human_review_required_for,
        &["peer_reviewed"],
        "missing_human_review_gate",
        "/human_review_required_for",
    )?;
    validate_proposed_tasks(&graph.proposed_tasks)?;
    Ok(())
}

pub fn validate_plan_verification(value: &Value) -> Result<(), SchemaError> {
    let verification: PlanVerification = deserialize(SchemaName::PlanVerification, value)?;
    require_prefix(
        SchemaName::PlanVerification,
        &verification.verification_id,
        "pverify_",
        "/verification_id",
    )?;
    require_eq(
        SchemaName::PlanVerification,
        verification.verification_task_id == "pvtask_energy_001_a",
        "verification_task_lineage_mismatch",
        "/verification_task_id",
    )?;
    require_eq(
        SchemaName::PlanVerification,
        verification.proposal_id == "plan_energy_001_a",
        "verification_proposal_lineage_mismatch",
        "/proposal_id",
    )?;
    require_eq(
        SchemaName::PlanVerification,
        verification.verifier_runner_id == "actor_verifier_001",
        "verification_runner_lineage_mismatch",
        "/verifier_runner_id",
    )?;
    require_eq(
        SchemaName::PlanVerification,
        verification.verification_type == "plan_schema_policy_cross_check",
        "invalid_verification_type",
        "/verification_type",
    )?;
    require_eq(
        SchemaName::PlanVerification,
        verification.status == "submitted",
        "invalid_verification_status",
        "/status",
    )?;
    validate_plan_verification_findings(&verification)?;
    require_exact_set(
        SchemaName::PlanVerification,
        &verification.checked_items,
        &[
            "required_artifacts_present",
            "human_review_gate_present",
            "no_student_grading",
            "no_credential_handling",
            "no_arbitrary_prompt",
            "validation_plan_present",
        ],
        "invalid_checked_items",
        "/checked_items",
    )?;
    require_eq(
        SchemaName::PlanVerification,
        verification.authority == "advisory_only",
        "invalid_authority",
        "/authority",
    )?;
    Ok(())
}

fn validate_plan_verification_findings(verification: &PlanVerification) -> Result<(), SchemaError> {
    require_eq(
        SchemaName::PlanVerification,
        verification.findings.len() <= 20,
        "too_many_findings",
        "/findings",
    )?;
    for finding in &verification.findings {
        require_eq(
            SchemaName::PlanVerification,
            valid_ascii_slug(&finding.check_name),
            "invalid_finding_check_name",
            "/findings/check_name",
        )?;
        require_eq(
            SchemaName::PlanVerification,
            matches!(finding.severity.as_str(), "blocking" | "warning"),
            "invalid_finding_severity",
            "/findings/severity",
        )?;
        require_eq(
            SchemaName::PlanVerification,
            (1..=500).contains(&finding.message.chars().count()),
            "invalid_finding_message",
            "/findings/message",
        )?;
        validate_plan_verification_finding_message(&finding.message)?;
    }

    match verification.outcome.as_str() {
        "no_blocking_findings" => require_eq(
            SchemaName::PlanVerification,
            verification.findings.is_empty(),
            "findings_not_allowed_for_success",
            "/findings",
        ),
        "warnings_only" => require_eq(
            SchemaName::PlanVerification,
            !verification.findings.is_empty()
                && verification
                    .findings
                    .iter()
                    .all(|finding| finding.severity == "warning"),
            "invalid_warning_findings",
            "/findings",
        ),
        "blocking_findings" => require_eq(
            SchemaName::PlanVerification,
            !verification.findings.is_empty()
                && verification
                    .findings
                    .iter()
                    .all(|finding| finding.severity == "blocking"),
            "invalid_blocking_findings",
            "/findings",
        ),
        _ => Err(SchemaError::new(
            SchemaName::PlanVerification,
            "invalid_verification_outcome",
            "/outcome",
        )),
    }
}

fn validate_plan_verification_finding_message(value: &str) -> Result<(), SchemaError> {
    validate_safe_text(SchemaName::PlanVerification, value, "/findings/message")?;
    require_eq(
        SchemaName::PlanVerification,
        !contains_plan_verification_authority_claim(value),
        "unsafe_finding_message",
        "/findings/message",
    )
}

fn contains_plan_verification_authority_claim(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("human approval")
        || lower.contains("human approved")
        || lower.contains("peer reviewed")
        || lower.contains("peer_reviewed")
        || lower.contains("promote")
        || lower.contains("promotion")
        || lower.contains("publish")
        || lower.contains("publication")
        || lower.contains("state override")
        || lower.contains("override state")
        || lower.contains("accepted verification")
}

pub fn validate_artifact_manifest(value: &Value) -> Result<(), SchemaError> {
    let manifest: ArtifactManifest = deserialize(SchemaName::ArtifactManifest, value)?;
    require_prefix(
        SchemaName::ArtifactManifest,
        &manifest.artifact_id,
        "art_",
        "/artifact_id",
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.request_id == "req_energy_001",
        "artifact_request_lineage_mismatch",
        "/request_id",
    )?;
    require_exact_set(
        SchemaName::ArtifactManifest,
        &manifest.work_packet_ids,
        &["wp_energy_001_generate_pack"],
        "invalid_work_packet_ids",
        "/work_packet_ids",
    )?;
    validate_safe_text(SchemaName::ArtifactManifest, &manifest.title, "/title")?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.subject == "physics",
        "invalid_subject",
        "/subject",
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.topic == "conservation_of_energy",
        "invalid_topic",
        "/topic",
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.age_range == "14-16",
        "invalid_age_range",
        "/age_range",
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.language == "en",
        "invalid_language",
        "/language",
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.license == "CC-BY-4.0",
        "invalid_license",
        "/license",
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.status_claim == "draft_generated",
        "invalid_status_claim",
        "/status_claim",
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        manifest.ai_assisted,
        "invalid_ai_assistance_disclosure",
        "/ai_assisted",
    )?;
    require_exact_set(
        SchemaName::ArtifactManifest,
        &manifest.contents,
        MANIFEST_CONTENTS,
        "invalid_manifest_contents",
        "/contents",
    )?;
    validate_safe_text_list(
        SchemaName::ArtifactManifest,
        &manifest.known_limitations,
        "/known_limitations",
        8,
    )?;
    require_eq(
        SchemaName::ArtifactManifest,
        !manifest.known_limitations.is_empty(),
        "known_limitations_required",
        "/known_limitations",
    )?;
    Ok(())
}

fn require_schema_files(root: &Path) -> Result<(), SchemaError> {
    let schema_dir = root.join("schemas");
    for (schema_name, file_name) in [
        (SchemaName::MvpRequest, REQUEST_SCHEMA),
        (
            SchemaName::RequestModerationReport,
            REQUEST_MODERATION_REPORT_SCHEMA,
        ),
        (SchemaName::ProposedTaskGraph, PROPOSED_TASK_GRAPH_SCHEMA),
        (SchemaName::PlanVerification, PLAN_VERIFICATION_SCHEMA),
        (SchemaName::ArtifactManifest, ARTIFACT_MANIFEST_SCHEMA),
    ] {
        let path = schema_dir.join(file_name);
        if !path.is_file() {
            return Err(SchemaError::new(
                schema_name,
                "schema_file_missing",
                "/schemas",
            ));
        }
        let schema = read_json(&path, schema_name)
            .map_err(|_| SchemaError::new(schema_name, "schema_json_invalid", "/schemas"))?;
        validate_schema_document(schema_name, &schema)?;
    }
    Ok(())
}

fn require_exact_fixture_files(root: &Path) -> Result<(), SchemaError> {
    require_exact_dir_files(
        &root.join("schemas"),
        &[
            REQUEST_SCHEMA,
            REQUEST_MODERATION_REPORT_SCHEMA,
            PROPOSED_TASK_GRAPH_SCHEMA,
            PLAN_VERIFICATION_SCHEMA,
            ARTIFACT_MANIFEST_SCHEMA,
        ],
        SchemaName::MvpRequest,
        "unexpected_schema_file",
        "/schemas",
    )?;
    require_exact_dir_files(
        &root.join("examples").join("mvp"),
        &[
            "request.valid.json",
            "request_moderation_report.valid.json",
            "proposed_task_graph.valid.json",
            "plan_verification.valid.json",
            "artifact_manifest.valid.json",
        ],
        SchemaName::MvpRequest,
        "unexpected_fixture_file",
        "/examples/mvp",
    )
}

fn require_exact_dir_files(
    directory: &Path,
    expected: &[&str],
    schema: SchemaName,
    code: &'static str,
    field_path: &'static str,
) -> Result<(), SchemaError> {
    let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(directory)
        .map_err(|_| SchemaError::new(schema, "fixture_directory_missing", field_path))?
    {
        let entry = entry.map_err(|_| SchemaError::new(schema, code, field_path))?;
        if !entry
            .file_type()
            .map_err(|_| SchemaError::new(schema, code, field_path))?
            .is_file()
        {
            return Err(SchemaError::new(schema, code, field_path));
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Err(SchemaError::new(schema, code, field_path));
        };
        actual.insert(name);
    }
    let actual_refs: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
    require_eq(schema, actual_refs == expected_set, code, field_path)
}

fn validate_against_schema(
    root: &Path,
    schema_file: &str,
    schema: SchemaName,
    fixture: &Value,
) -> Result<(), SchemaError> {
    let schema_value = read_json(&root.join("schemas").join(schema_file), schema)
        .map_err(|_| SchemaError::new(schema, "schema_json_invalid", "/schemas"))?;
    validate_schema_document(schema, &schema_value)?;
    validate_schema_value(schema, &schema_value, &schema_value, fixture, "/")
}

fn validate_schema_document(schema: SchemaName, value: &Value) -> Result<(), SchemaError> {
    let Some(object) = value.as_object() else {
        return Err(SchemaError::new(
            schema,
            "schema_compile_failed",
            "/schemas",
        ));
    };
    require_eq(
        schema,
        object.get("type").and_then(Value::as_str) == Some("object"),
        "schema_compile_failed",
        "/schemas",
    )?;
    reject_unsupported_schema_keywords(schema, value, false)
}

fn reject_unsupported_schema_keywords(
    schema: SchemaName,
    value: &Value,
    allow_partial_object_schema: bool,
) -> Result<(), SchemaError> {
    match value {
        Value::Object(object) => {
            for keyword in object.keys() {
                if !schema_keyword_is_supported(keyword) {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                }
            }
            let has_supported_assertion = [
                "type",
                "const",
                "enum",
                "anyOf",
                "allOf",
                "pattern",
                "properties",
                "items",
                "prefixItems",
                "$ref",
            ]
            .iter()
            .any(|keyword| object.contains_key(*keyword));
            if !has_supported_assertion {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            if let Some(required_value) = object.get("required") {
                let Some(required) = required_value.as_array() else {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                };
                if !required.iter().all(Value::is_string) {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                }
            }
            if object
                .get("additionalProperties")
                .is_some_and(|additional| !additional.is_boolean())
            {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            if object.get("type").and_then(Value::as_str) == Some("object")
                && object.get("additionalProperties").and_then(Value::as_bool) != Some(false)
            {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            for keyword in ["minItems", "maxItems", "minLength", "maxLength"] {
                if object
                    .get(keyword)
                    .is_some_and(|numeric_value| numeric_value.as_u64().is_none())
                {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                }
            }
            if object
                .get("uniqueItems")
                .is_some_and(|unique_items| !unique_items.is_boolean())
            {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            if let Some(type_value) = object.get("type") {
                let Some(type_name) = type_value.as_str() else {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                };
                if !matches!(
                    type_name,
                    "object" | "array" | "string" | "integer" | "boolean"
                ) {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                }
            }
            require_type_for_keyword_family(
                schema,
                object,
                "object",
                &["properties", "required", "additionalProperties"],
                allow_partial_object_schema,
            )?;
            require_type_for_keyword_family(
                schema,
                object,
                "array",
                &[
                    "items",
                    "prefixItems",
                    "minItems",
                    "maxItems",
                    "uniqueItems",
                ],
                false,
            )?;
            require_type_for_keyword_family(
                schema,
                object,
                "string",
                &["pattern", "minLength", "maxLength"],
                false,
            )?;
            if object
                .get("anyOf")
                .is_some_and(|any_of| any_of.as_array().is_none_or(|items| items.is_empty()))
            {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            if object
                .get("allOf")
                .is_some_and(|all_of| all_of.as_array().is_none_or(|items| items.is_empty()))
            {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            if object.get("$ref").is_some_and(|reference| {
                reference
                    .as_str()
                    .is_none_or(|reference| !reference.starts_with("#/$defs/"))
            }) {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            if object
                .get("enum")
                .is_some_and(|enum_value| enum_value.as_array().is_none())
            {
                return Err(SchemaError::new(
                    schema,
                    "schema_compile_failed",
                    "/schemas",
                ));
            }
            if let Some(properties_value) = object.get("properties") {
                let Some(properties) = properties_value.as_object() else {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                };
                for child in properties.values() {
                    reject_unsupported_schema_keywords(schema, child, false)?;
                }
            }
            if let Some(items) = object.get("items") {
                if !items.is_object() {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                }
                reject_unsupported_schema_keywords(schema, items, false)?;
            }
            if let Some(prefix_items_value) = object.get("prefixItems") {
                let Some(prefix_items) = prefix_items_value.as_array() else {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                };
                if prefix_items.is_empty() {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                }
                for child in prefix_items {
                    reject_unsupported_schema_keywords(schema, child, false)?;
                }
            }
            if let Some(defs_value) = object.get("$defs") {
                let Some(defs) = defs_value.as_object() else {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                };
                for child in defs.values() {
                    reject_unsupported_schema_keywords(schema, child, false)?;
                }
            }
            if let Some(any_of) = object.get("anyOf").and_then(Value::as_array) {
                for child in any_of {
                    reject_unsupported_schema_keywords(schema, child, true)?;
                }
            }
            if let Some(all_of) = object.get("allOf").and_then(Value::as_array) {
                for child in all_of {
                    reject_unsupported_schema_keywords(schema, child, true)?;
                }
            }
            if let Some(pattern_value) = object.get("pattern") {
                let Some(pattern) = pattern_value.as_str() else {
                    return Err(SchemaError::new(
                        schema,
                        "schema_compile_failed",
                        "/schemas",
                    ));
                };
                if !schema_pattern_is_supported(pattern) {
                    return Err(SchemaError::new(
                        schema,
                        "unsupported_schema_pattern",
                        "/schemas",
                    ));
                }
            }
            Ok(())
        }
        Value::Array(_) | Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            Err(SchemaError::new(
                schema,
                "schema_compile_failed",
                "/schemas",
            ))
        }
    }
}

fn require_type_for_keyword_family(
    schema: SchemaName,
    object: &serde_json::Map<String, Value>,
    required_type: &'static str,
    keywords: &[&str],
    allow_missing_type: bool,
) -> Result<(), SchemaError> {
    if keywords.iter().any(|keyword| object.contains_key(*keyword))
        && match object.get("type").and_then(Value::as_str) {
            Some(actual_type) => actual_type != required_type,
            None => !allow_missing_type,
        }
    {
        return Err(SchemaError::new(
            schema,
            "schema_compile_failed",
            "/schemas",
        ));
    }
    Ok(())
}

fn schema_keyword_is_supported(keyword: &str) -> bool {
    matches!(
        keyword,
        "$schema"
            | "title"
            | "description"
            | "$defs"
            | "$ref"
            | "type"
            | "const"
            | "enum"
            | "anyOf"
            | "allOf"
            | "pattern"
            | "properties"
            | "items"
            | "prefixItems"
            | "additionalProperties"
            | "required"
            | "minItems"
            | "maxItems"
            | "uniqueItems"
            | "minLength"
            | "maxLength"
    )
}

fn validate_schema_value(
    schema_name: SchemaName,
    root_schema: &Value,
    schema: &Value,
    value: &Value,
    field_path: &str,
) -> Result<(), SchemaError> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let Some(referenced_schema) = resolve_schema_ref(root_schema, reference) else {
            return Err(SchemaError::new(
                schema_name,
                "schema_compile_failed",
                "/schemas",
            ));
        };
        return validate_schema_value(
            schema_name,
            root_schema,
            referenced_schema,
            value,
            field_path,
        );
    }
    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for option in all_of {
            validate_schema_value(schema_name, root_schema, option, value, field_path)?;
        }
    }
    if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
        let mut matched = false;
        for option in any_of {
            match validate_schema_value(schema_name, root_schema, option, value, field_path) {
                Ok(()) => matched = true,
                Err(error) if error.code == "schema_compile_failed" => return Err(error),
                Err(_) => {}
            }
        }
        if !matched {
            return Err(SchemaError::new(
                schema_name,
                "fixture_schema_validation_failed",
                field_path,
            ));
        }
    }
    if let Some(expected_const) = schema.get("const") {
        return require_eq(
            schema_name,
            value == expected_const,
            "fixture_schema_validation_failed",
            field_path,
        );
    }
    if let Some(enums) = schema.get("enum").and_then(Value::as_array) {
        return require_eq(
            schema_name,
            enums.iter().any(|item| item == value),
            "fixture_schema_validation_failed",
            field_path,
        );
    }

    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            validate_schema_object(schema_name, root_schema, schema, value, field_path)
        }
        Some("array") => validate_schema_array(schema_name, root_schema, schema, value, field_path),
        Some("string") => validate_schema_string(schema_name, schema, value, field_path),
        Some("integer") => require_eq(
            schema_name,
            value.as_i64().is_some(),
            "fixture_schema_validation_failed",
            field_path,
        ),
        Some("boolean") => require_eq(
            schema_name,
            value.as_bool().is_some(),
            "fixture_schema_validation_failed",
            field_path,
        ),
        _ if schema.get("properties").is_some() || schema.get("required").is_some() => {
            validate_schema_object(schema_name, root_schema, schema, value, field_path)
        }
        _ if schema.get("items").is_some() || schema.get("prefixItems").is_some() => {
            validate_schema_array(schema_name, root_schema, schema, value, field_path)
        }
        _ => Ok(()),
    }
}

fn resolve_schema_ref<'a>(root_schema: &'a Value, reference: &str) -> Option<&'a Value> {
    let name = reference.strip_prefix("#/$defs/")?;
    root_schema.get("$defs")?.get(name)
}

fn validate_schema_object(
    schema_name: SchemaName,
    root_schema: &Value,
    schema: &Value,
    value: &Value,
    field_path: &str,
) -> Result<(), SchemaError> {
    let Some(object) = value.as_object() else {
        return Err(SchemaError::new(
            schema_name,
            "fixture_schema_validation_failed",
            field_path,
        ));
    };
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for field in required {
            let Some(field_name) = field.as_str() else {
                return Err(SchemaError::new(
                    schema_name,
                    "schema_compile_failed",
                    "/schemas",
                ));
            };
            if !object.contains_key(field_name) {
                return Err(SchemaError::new(
                    schema_name,
                    "fixture_schema_validation_failed",
                    field_path,
                ));
            }
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
        for key in object.keys() {
            if !properties.is_some_and(|properties| properties.contains_key(key)) {
                return Err(SchemaError::new(
                    schema_name,
                    "fixture_schema_validation_failed",
                    field_path,
                ));
            }
        }
    }
    if let Some(properties) = properties {
        for (key, property_schema) in properties {
            if let Some(nested) = object.get(key) {
                let child_path = json_pointer_child(field_path, key);
                validate_schema_value(
                    schema_name,
                    root_schema,
                    property_schema,
                    nested,
                    &child_path,
                )?;
            }
        }
    }
    Ok(())
}

fn validate_schema_array(
    schema_name: SchemaName,
    root_schema: &Value,
    schema: &Value,
    value: &Value,
    field_path: &str,
) -> Result<(), SchemaError> {
    let Some(items) = value.as_array() else {
        return Err(SchemaError::new(
            schema_name,
            "fixture_schema_validation_failed",
            field_path,
        ));
    };
    if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64) {
        require_eq(
            schema_name,
            items.len() >= min_items as usize,
            "fixture_schema_validation_failed",
            field_path,
        )?;
    }
    if let Some(max_items) = schema.get("maxItems").and_then(Value::as_u64) {
        require_eq(
            schema_name,
            items.len() <= max_items as usize,
            "fixture_schema_validation_failed",
            field_path,
        )?;
    }
    if schema.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
        let mut seen = BTreeSet::new();
        for item in items {
            let encoded = serde_json::to_string(item).map_err(|_| {
                SchemaError::new(schema_name, "fixture_schema_validation_failed", field_path)
            })?;
            if !seen.insert(encoded) {
                return Err(SchemaError::new(
                    schema_name,
                    "fixture_schema_validation_failed",
                    field_path,
                ));
            }
        }
    }
    if let Some(prefix_items) = schema.get("prefixItems").and_then(Value::as_array) {
        for (index, (item, item_schema)) in items.iter().zip(prefix_items).enumerate() {
            let child_path = json_pointer_child(field_path, &index.to_string());
            validate_schema_value(schema_name, root_schema, item_schema, item, &child_path)?;
        }
    } else if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            let child_path = json_pointer_child(field_path, &index.to_string());
            validate_schema_value(schema_name, root_schema, item_schema, item, &child_path)?;
        }
    }
    Ok(())
}

fn validate_schema_string(
    schema_name: SchemaName,
    schema: &Value,
    value: &Value,
    field_path: &str,
) -> Result<(), SchemaError> {
    let Some(text) = value.as_str() else {
        return Err(SchemaError::new(
            schema_name,
            "fixture_schema_validation_failed",
            field_path,
        ));
    };
    if let Some(min_length) = schema.get("minLength").and_then(Value::as_u64) {
        require_eq(
            schema_name,
            text.chars().count() >= min_length as usize,
            "fixture_schema_validation_failed",
            field_path,
        )?;
    }
    if let Some(max_length) = schema.get("maxLength").and_then(Value::as_u64) {
        require_eq(
            schema_name,
            text.chars().count() <= max_length as usize,
            "fixture_schema_validation_failed",
            field_path,
        )?;
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        require_eq(
            schema_name,
            schema_pattern_matches(pattern, text),
            "fixture_schema_validation_failed",
            field_path,
        )?;
    }
    Ok(())
}

fn schema_pattern_matches(pattern: &str, text: &str) -> bool {
    match pattern {
        "^plan_[a-z0-9_-]+$" => text.strip_prefix("plan_").is_some_and(valid_ascii_slug),
        "^actor_[a-z0-9_-]+$" => text.strip_prefix("actor_").is_some_and(valid_ascii_slug),
        "^pverify_[a-z0-9_-]+$" => text.strip_prefix("pverify_").is_some_and(valid_ascii_slug),
        "^no_arbitrary_[a-z]{6}$" => text.strip_prefix("no_arbitrary_").is_some_and(|suffix| {
            suffix.len() == 6
                && suffix
                    .chars()
                    .all(|character| character.is_ascii_lowercase())
        }),
        PLAN_VERIFICATION_SAFE_FINDING_MESSAGE_PATTERN => {
            (1..=500).contains(&text.chars().count())
                && !contains_unsafe_safe_text_marker(text)
                && !contains_plan_verification_authority_claim(text)
                && !text.contains('\0')
                && !text.chars().any(char::is_control)
        }
        "^[a-z0-9_-]+$" => valid_ascii_slug(text),
        _ => false,
    }
}

fn schema_pattern_is_supported(pattern: &str) -> bool {
    matches!(
        pattern,
        "^plan_[a-z0-9_-]+$"
            | "^actor_[a-z0-9_-]+$"
            | "^pverify_[a-z0-9_-]+$"
            | "^no_arbitrary_[a-z]{6}$"
            | PLAN_VERIFICATION_SAFE_FINDING_MESSAGE_PATTERN
            | "^[a-z0-9_-]+$"
    )
}

fn valid_ascii_slug(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        })
}

fn read_json(path: &Path, schema: SchemaName) -> Result<Value, SchemaError> {
    let bytes = fs::read(path).map_err(|_| SchemaError::new(schema, "fixture_missing", "/"))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| SchemaError::new(schema, "fixture_json_invalid", "/"))
}

fn deserialize<T: DeserializeOwned>(schema: SchemaName, value: &Value) -> Result<T, SchemaError> {
    serde_json::from_value(value.clone()).map_err(|error| {
        if error.to_string().contains("unknown field") {
            SchemaError::new(schema, "unknown_field", "/")
        } else {
            SchemaError::new(schema, "invalid_shape", "/")
        }
    })
}

fn validate_request_summary(summary: &SourceRequestSummary) -> Result<(), SchemaError> {
    require_eq(
        SchemaName::ProposedTaskGraph,
        summary.subject == "physics",
        "proposal_request_summary_mismatch",
        "/source_request_summary/subject",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        summary.topic == "conservation_of_energy",
        "proposal_request_summary_mismatch",
        "/source_request_summary/topic",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        summary.age_range == "14-16",
        "proposal_request_summary_mismatch",
        "/source_request_summary/age_range",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        summary.duration_minutes == 45,
        "proposal_request_summary_mismatch",
        "/source_request_summary/duration_minutes",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        summary.language == "en",
        "proposal_request_summary_mismatch",
        "/source_request_summary/language",
    )?;
    Ok(())
}

fn validate_proposed_artifacts(artifacts: &[ProposedArtifact]) -> Result<(), SchemaError> {
    let required: Vec<String> = artifacts
        .iter()
        .filter(|artifact| artifact.priority == "required")
        .map(|artifact| artifact.artifact_type.clone())
        .collect();
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &required,
        DESIRED_ARTIFACTS,
        "invalid_proposed_artifacts",
        "/proposed_artifacts",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        artifacts.len() == DESIRED_ARTIFACTS.len(),
        "invalid_proposed_artifacts",
        "/proposed_artifacts",
    )
}

fn validate_proposed_tasks(tasks: &[ProposedTask]) -> Result<(), SchemaError> {
    require_eq(
        SchemaName::ProposedTaskGraph,
        tasks.len() == 3,
        "invalid_proposed_tasks",
        "/proposed_tasks",
    )?;
    validate_local_ids_and_dependencies(tasks)?;
    let generation = task_by_type(tasks, "generate_lesson_pack")?;
    let validation = task_by_type(tasks, "run_artifact_validation")?;
    let review = task_by_type(tasks, "review_subject_and_pedagogy")?;

    validate_task_request_fields(generation)?;
    validate_task_request_fields(validation)?;
    validate_task_request_fields(review)?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        generation.phase == "initial_generation" && generation.depends_on.is_empty(),
        "invalid_generation_task",
        "/proposed_tasks",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &generation.required_capabilities,
        &["stem_pedagogy", "structured_markdown", "basic_python"],
        "invalid_generation_capabilities",
        "/proposed_tasks/required_capabilities",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &generation.outputs,
        MANIFEST_CONTENTS,
        "invalid_generation_outputs",
        "/proposed_tasks/outputs",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &generation.validation_required,
        VALIDATION_CHECKS,
        "invalid_generation_validation",
        "/proposed_tasks/validation_required",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &generation.human_review_required_for,
        &["peer_reviewed"],
        "missing_human_review_gate",
        "/proposed_tasks/human_review_required_for",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        generation
            .execution_policy
            .as_deref()
            .is_none_or(|policy| policy == "code_generation_only")
            && validation.execution_policy.is_none()
            && review.execution_policy.is_none(),
        "invalid_execution_policy",
        "/proposed_tasks/execution_policy",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        validation.phase == "mechanical_validation"
            && validation.depends_on == [generation.local_id.as_str()],
        "invalid_validation_task",
        "/proposed_tasks",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &validation.required_capabilities,
        &["artifact_validation", "python_execution_limited"],
        "invalid_validation_capabilities",
        "/proposed_tasks/required_capabilities",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        validation.validation_required.is_empty()
            && validation.human_review_required_for.is_empty(),
        "invalid_validation_task",
        "/proposed_tasks",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &validation.outputs,
        &["validation_report.json"],
        "invalid_validation_outputs",
        "/proposed_tasks/outputs",
    )?;
    require_eq(
        SchemaName::ProposedTaskGraph,
        review.phase == "human_review" && review.depends_on == [validation.local_id.as_str()],
        "invalid_review_task",
        "/proposed_tasks",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &review.required_capabilities,
        &["human_subject_review", "human_pedagogy_review"],
        "invalid_review_capabilities",
        "/proposed_tasks/required_capabilities",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &review.human_review_required_for,
        &["peer_reviewed"],
        "missing_human_review_gate",
        "/proposed_tasks/human_review_required_for",
    )?;
    require_exact_set(
        SchemaName::ProposedTaskGraph,
        &review.outputs,
        &["review.json"],
        "invalid_review_outputs",
        "/proposed_tasks/outputs",
    )?;
    Ok(())
}

fn validate_local_ids_and_dependencies(tasks: &[ProposedTask]) -> Result<(), SchemaError> {
    let mut local_ids = BTreeSet::new();
    for task in tasks {
        require_eq(
            SchemaName::ProposedTaskGraph,
            valid_local_id(&task.local_id),
            "invalid_task_local_id",
            "/proposed_tasks/local_id",
        )?;
        if !local_ids.insert(task.local_id.as_str()) {
            return Err(SchemaError::new(
                SchemaName::ProposedTaskGraph,
                "duplicate_task_local_id",
                "/proposed_tasks/local_id",
            ));
        }
        let mut deps = BTreeSet::new();
        for dependency in &task.depends_on {
            if dependency == &task.local_id {
                return Err(SchemaError::new(
                    SchemaName::ProposedTaskGraph,
                    "self_task_dependency",
                    "/proposed_tasks/depends_on",
                ));
            }
            if !deps.insert(dependency.as_str()) {
                return Err(SchemaError::new(
                    SchemaName::ProposedTaskGraph,
                    "duplicate_task_dependency",
                    "/proposed_tasks/depends_on",
                ));
            }
        }
    }
    for task in tasks {
        for dependency in &task.depends_on {
            if !local_ids.contains(dependency.as_str()) {
                return Err(SchemaError::new(
                    SchemaName::ProposedTaskGraph,
                    "unknown_task_dependency",
                    "/proposed_tasks/depends_on",
                ));
            }
        }
    }
    Ok(())
}

fn valid_local_id(local_id: &str) -> bool {
    !local_id.is_empty()
        && local_id.len() <= 64
        && !local_id.starts_with("system_")
        && !local_id.starts_with("admin_")
        && !local_id.starts_with("credential_")
        && local_id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        })
}

fn task_by_type<'a>(
    tasks: &'a [ProposedTask],
    task_type: &str,
) -> Result<&'a ProposedTask, SchemaError> {
    tasks
        .iter()
        .find(|task| task.task_type == task_type)
        .ok_or_else(|| {
            SchemaError::new(
                SchemaName::ProposedTaskGraph,
                "missing_mvp_task",
                "/proposed_tasks",
            )
        })
}

fn validate_task_request_fields(task: &ProposedTask) -> Result<(), SchemaError> {
    require_eq(
        SchemaName::ProposedTaskGraph,
        task.subject == "physics"
            && task.topic == "conservation_of_energy"
            && task.age_range == "14-16"
            && task.language == "en"
            && task.risk_level == "low",
        "proposal_task_request_mismatch",
        "/proposed_tasks",
    )
}

fn validate_safe_text(
    schema: SchemaName,
    value: &str,
    field_path: &'static str,
) -> Result<(), SchemaError> {
    // Fixture schemas use this deliberately broad deterministic heuristic to keep
    // raw private values out of checked-in examples; errors expose only safe codes
    // and field paths, never the rejected value.
    let unsafe_value = contains_unsafe_safe_text_marker(value)
        || value.contains('\0')
        || value.chars().any(char::is_control);
    require_eq(schema, !unsafe_value, "unsafe_text_value", field_path)
}

fn contains_unsafe_safe_text_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let has_windows_drive_path = value.as_bytes().windows(3).any(|window| {
        window[0].is_ascii_alphabetic() && window[1] == b':' && matches!(window[2], b'\\' | b'/')
    });
    let has_unc_path = value.as_bytes().windows(2).any(|window| window == b"\\\\");
    let has_local_path = value.contains("/Users/")
        || value.contains("/home/")
        || value.contains("/etc/")
        || value.contains("/private/")
        || value.contains("/var/")
        || value.contains("/tmp/")
        || has_windows_drive_path
        || has_unc_path
        || value.contains("../")
        || value.contains("~/")
        || lower.contains("ssh/");
    value.contains("://")
        || value.contains('@')
        || lower.contains("secret")
        || lower.contains("api_key")
        || lower.contains("token")
        || lower.contains("cookie")
        || lower.contains("credential")
        || lower.contains("password")
        || lower.contains("student record")
        || lower.contains("student grade")
        || lower.contains("student placement")
        || lower.contains("student profile")
        || lower.contains("placement decision")
        || lower.contains("disciplinary record")
        || lower.contains("disciplinary action")
        || has_local_path
}

fn validate_safe_text_list(
    schema: SchemaName,
    values: &[String],
    field_path: &'static str,
    max_items: usize,
) -> Result<(), SchemaError> {
    require_eq(
        schema,
        values.len() <= max_items,
        "too_many_items",
        field_path,
    )?;
    for value in values {
        require_eq(
            schema,
            value.chars().count() <= 240,
            "text_too_long",
            field_path,
        )?;
        validate_safe_text(schema, value, field_path)?;
    }
    Ok(())
}

fn require_prefix(
    schema: SchemaName,
    value: &str,
    prefix: &str,
    field_path: &'static str,
) -> Result<(), SchemaError> {
    require_eq(
        schema,
        value.starts_with(prefix),
        "invalid_identifier",
        field_path,
    )
}

fn require_exact_set(
    schema: SchemaName,
    actual: &[String],
    expected: &[&str],
    code: &'static str,
    field_path: &'static str,
) -> Result<(), SchemaError> {
    let actual_set: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
    let expected_set: BTreeSet<&str> = expected.iter().copied().collect();
    require_eq(
        schema,
        actual.len() == expected.len() && actual_set == expected_set,
        code,
        field_path,
    )
}

fn require_allowed_subset(
    schema: SchemaName,
    actual: &[String],
    allowed: &[&str],
    code: &'static str,
    field_path: &'static str,
) -> Result<(), SchemaError> {
    let allowed_set: BTreeSet<&str> = allowed.iter().copied().collect();
    let actual_set: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
    require_eq(
        schema,
        !actual.is_empty()
            && actual.len() == actual_set.len()
            && actual_set.iter().all(|item| allowed_set.contains(item)),
        code,
        field_path,
    )
}

fn require_eq(
    schema: SchemaName,
    condition: bool,
    code: &'static str,
    field_path: impl Into<String>,
) -> Result<(), SchemaError> {
    if condition {
        Ok(())
    } else {
        Err(SchemaError::new(schema, code, field_path))
    }
}

fn json_pointer_child(parent: &str, child: &str) -> String {
    let escaped = child.replace('~', "~0").replace('/', "~1");
    if parent == "/" {
        format!("/{escaped}")
    } else {
        format!("{parent}/{escaped}")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MvpRequest {
    title: String,
    subject: String,
    topic: String,
    age_range: String,
    language: String,
    lesson_duration_minutes: u16,
    desired_artifacts: Vec<String>,
    constraints: Vec<String>,
    license_preference: String,
    visibility: String,
    forbidden_content_acknowledged: bool,
    auto_repair_preference: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestModerationReport {
    request_moderation_report_id: String,
    request_moderation_task_id: String,
    request_id: String,
    lease_id: String,
    moderation_kind: String,
    decision: String,
    category_flags: Vec<String>,
    safe_reason_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedTaskGraph {
    proposal_id: String,
    request_id: String,
    planning_task_id: String,
    planner_runner_id: String,
    schema_version: String,
    status: String,
    source_request_summary: SourceRequestSummary,
    assumptions: Vec<String>,
    missing_information: Vec<String>,
    proposed_artifacts: Vec<ProposedArtifact>,
    proposed_tasks: Vec<ProposedTask>,
    validation_plan: Vec<String>,
    human_review_required_for: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRequestSummary {
    subject: String,
    topic: String,
    age_range: String,
    duration_minutes: u16,
    language: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedArtifact {
    artifact_type: String,
    priority: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedTask {
    local_id: String,
    phase: String,
    task_type: String,
    subject: String,
    topic: String,
    age_range: String,
    language: String,
    risk_level: String,
    #[serde(default)]
    depends_on: Vec<String>,
    required_capabilities: Vec<String>,
    outputs: Vec<String>,
    validation_required: Vec<String>,
    human_review_required_for: Vec<String>,
    #[serde(default)]
    execution_policy: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanVerification {
    verification_id: String,
    proposal_id: String,
    verification_task_id: String,
    verifier_runner_id: String,
    verification_type: String,
    status: String,
    outcome: String,
    findings: Vec<PlanVerificationFinding>,
    checked_items: Vec<String>,
    authority: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanVerificationFinding {
    check_name: String,
    severity: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactManifest {
    artifact_id: String,
    request_id: String,
    work_packet_ids: Vec<String>,
    title: String,
    subject: String,
    topic: String,
    age_range: String,
    language: String,
    license: String,
    status_claim: String,
    ai_assisted: bool,
    contents: Vec<String>,
    known_limitations: Vec<String>,
}
