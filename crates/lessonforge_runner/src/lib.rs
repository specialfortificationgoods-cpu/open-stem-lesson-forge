use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

const MAX_ED25519_KEY_FILE_BYTES: u64 = 1024;
static SELF_TEST_REPORT_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn crate_boundary() -> &'static str {
    "local_runner"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum RunnerMode {
    DummyRequestModerator,
    DummyPlanner,
    DummyPlanVerifier,
    DummyGenerator,
    DummyCodeCritic,
    DummyCodeRepairer,
    DummyCodeRepairerAutoLoop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    pub runner: RunnerSection,
    pub auth: AuthSection,
    pub transport: TransportSection,
    pub attestation: AttestationSection,
    pub capabilities: CapabilityConfig,
    pub policy: RunnerPolicy,
    pub forbidden: ForbiddenPrivateFields,
}

impl RunnerConfig {
    pub fn dummy(
        runner_id: impl Into<String>,
        public_name: impl Into<String>,
        mode: RunnerMode,
        central_api_base: impl Into<String>,
    ) -> Self {
        Self {
            runner: RunnerSection {
                runner_id: runner_id.into(),
                public_name: public_name.into(),
                mode,
                scope_id: "scope_default".to_owned(),
                central_api_base: central_api_base.into(),
                workspace_root: ".lessonforge-runner-work".to_owned(),
            },
            auth: AuthSection {
                kind: AuthKind::BearerTokenEnv,
                token_env: "LESSONFORGE_RUNNER_TOKEN".to_owned(),
            },
            transport: TransportSection {
                tls: TlsSection {
                    trust_policy: TlsTrustPolicy::LoopbackDevelopment,
                    pinned_ca_pem_path: String::new(),
                    pinned_spki_sha256: String::new(),
                    expected_server_name: String::new(),
                },
            },
            attestation: AttestationSection {
                runner_key_id: String::new(),
                ed25519_private_key_path: String::new(),
                runner_private_key_dir: String::new(),
            },
            capabilities: CapabilityConfig::for_mode(mode),
            policy: RunnerPolicy {
                max_tasks_per_day_bucket: "1-5".to_owned(),
                auto_submit_status_cap: "draft_only".to_owned(),
                allowed_risk_level_max: "low".to_owned(),
                allow_provider_backed_modes: false,
                allow_tool_execution: false,
                automated_repair_loop_opt_in: false,
                max_automated_repair_attempts: 0,
            },
            forbidden: ForbiddenPrivateFields::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerSection {
    pub runner_id: String,
    pub public_name: String,
    pub mode: RunnerMode,
    pub scope_id: String,
    pub central_api_base: String,
    pub workspace_root: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    BearerTokenEnv,
    TestStaticToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSection {
    pub kind: AuthKind,
    pub token_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSection {
    pub tls: TlsSection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsTrustPolicy {
    LoopbackDevelopment,
    PinnedCa,
    PinnedSpki,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsSection {
    pub trust_policy: TlsTrustPolicy,
    pub pinned_ca_pem_path: String,
    pub pinned_spki_sha256: String,
    pub expected_server_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationSection {
    pub runner_key_id: String,
    pub ed25519_private_key_path: String,
    pub runner_private_key_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityConfig {
    pub subjects: Vec<String>,
    pub languages: Vec<String>,
    pub phases: Vec<String>,
    pub task_types: Vec<String>,
    pub workflow_capabilities: Vec<String>,
    pub artifact_types: Vec<String>,
    pub tools: Vec<String>,
}

impl CapabilityConfig {
    pub fn for_mode_for_test(mode: RunnerMode) -> Self {
        Self::for_mode(mode)
    }

    fn for_mode(mode: RunnerMode) -> Self {
        let (phases, task_types, workflow_capabilities, tools) = match mode {
            RunnerMode::DummyRequestModerator => (
                vec!["request_moderation"],
                vec!["moderate_request"],
                vec!["content_moderation", "age_appropriateness_classification"],
                vec!["structured_json_output"],
            ),
            RunnerMode::DummyPlanner => (
                vec!["request_planning"],
                vec!["propose_task_graph"],
                vec![
                    "request_interpretation",
                    "task_decomposition",
                    "policy_reasoning",
                ],
                vec!["structured_json_output"],
            ),
            RunnerMode::DummyPlanVerifier => (
                vec!["plan_verification"],
                vec!["verify_proposed_task_graph"],
                vec![
                    "policy_cross_check",
                    "plan_consistency_review",
                    "policy_reasoning",
                ],
                vec!["structured_json_output"],
            ),
            RunnerMode::DummyGenerator => (
                vec!["artifact_generation"],
                vec!["generate_lesson_pack"],
                vec![
                    "artifact_generation",
                    "stem_pedagogy",
                    "structured_markdown",
                    "basic_python",
                    "python_execution_limited",
                ],
                vec![
                    "structured_json_output",
                    "sandboxed_python_checker_self_test",
                ],
            ),
            RunnerMode::DummyCodeCritic => (
                vec!["code_critique"],
                vec!["critique_generated_code"],
                vec![
                    "artifact_validation",
                    "policy_cross_check",
                    "python_execution_limited",
                ],
                vec![
                    "structured_json_output",
                    "sandboxed_python_checker_critique",
                ],
            ),
            RunnerMode::DummyCodeRepairer | RunnerMode::DummyCodeRepairerAutoLoop => (
                vec!["code_repair"],
                vec!["repair_generated_code"],
                vec![
                    "artifact_generation",
                    "basic_python",
                    "python_execution_limited",
                ],
                vec!["structured_json_output", "sandboxed_python_checker_repair"],
            ),
        };
        Self {
            subjects: vec!["physics".to_owned()],
            languages: vec!["en".to_owned()],
            phases: strings(phases),
            task_types: strings(task_types),
            workflow_capabilities: strings(workflow_capabilities),
            artifact_types: strings(["worksheet", "answer_key", "python_checker", "teacher_notes"]),
            tools: strings(tools),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerPolicy {
    pub max_tasks_per_day_bucket: String,
    pub auto_submit_status_cap: String,
    pub allowed_risk_level_max: String,
    pub allow_provider_backed_modes: bool,
    pub allow_tool_execution: bool,
    pub automated_repair_loop_opt_in: bool,
    pub max_automated_repair_attempts: u8,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ForbiddenPrivateFields {
    pub provider_api_key_literal: Option<String>,
    pub provider_base_url: Option<String>,
    pub auth_path: Option<String>,
    pub cookie_path: Option<String>,
    pub browser_profile_path: Option<String>,
    pub local_prompt_template: Option<String>,
    pub exact_quota: Option<String>,
    pub hidden_network_destination: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRunnerConfig {
    config: RunnerConfig,
    canonical_private_key_path: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CapabilitySummary {
    pub runner_id: String,
    pub runner_public_name: String,
    pub capability_schema_version: String,
    pub capabilities: RedactedCapabilities,
    pub trust_level: String,
    pub policy_summary: RedactedPolicySummary,
}

impl fmt::Debug for CapabilitySummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilitySummary")
            .field("runner_id", &self.runner_id)
            .field("runner_public_name", &self.runner_public_name)
            .field("capability_schema_version", &self.capability_schema_version)
            .field("capabilities", &self.capabilities)
            .field("trust_level", &self.trust_level)
            .field("policy_summary", &self.policy_summary)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedCapabilities {
    pub subjects: Vec<String>,
    pub languages: Vec<String>,
    pub phases: Vec<String>,
    pub task_types: Vec<String>,
    pub workflow_capabilities: Vec<String>,
    pub artifact_types: Vec<String>,
    pub tools: Vec<String>,
    pub automated_repair_loop_opt_in: bool,
    pub automated_repair_attempt_bucket: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedPolicySummary {
    pub max_tasks_per_day_bucket: String,
    pub auto_submit_status_cap: String,
    pub allowed_risk_level_max: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RunnerConfigError {
    #[error("unsafe central API origin")]
    UnsafeCentralApiOrigin,
    #[error("provider-backed modes are unavailable")]
    ProviderBackedModesUnavailable,
    #[error("unsafe private config field")]
    UnsafePrivateConfigField,
    #[error("invalid automated repair policy")]
    InvalidAutomatedRepairPolicy,
    #[error("tool execution is unavailable")]
    ToolExecutionUnavailable,
    #[error("unsafe summary field")]
    UnsafeSummaryField,
    #[error("capability mode mismatch")]
    CapabilityModeMismatch,
    #[error("invalid attestation config")]
    InvalidAttestationConfig,
}

impl RunnerConfigError {
    pub fn safe_code(&self) -> &'static str {
        match self {
            Self::UnsafeCentralApiOrigin => "unsafe_central_api_origin",
            Self::ProviderBackedModesUnavailable => "provider_backed_modes_unavailable",
            Self::UnsafePrivateConfigField => "unsafe_private_config_field",
            Self::InvalidAutomatedRepairPolicy => "invalid_automated_repair_policy",
            Self::ToolExecutionUnavailable => "tool_execution_unavailable",
            Self::UnsafeSummaryField => "unsafe_summary_field",
            Self::CapabilityModeMismatch => "capability_mode_mismatch",
            Self::InvalidAttestationConfig => "invalid_attestation_config",
        }
    }
}

pub fn validate_runner_config(
    config: &RunnerConfig,
) -> Result<ValidatedRunnerConfig, RunnerConfigError> {
    validate_private_fields(config)?;
    validate_origin(&config.runner.central_api_base, &config.transport.tls)?;
    validate_summary_fields(config)?;
    validate_capabilities_match_mode(config)?;
    let canonical_private_key_path = validate_attestation_config(config)?;
    if config.policy.allow_provider_backed_modes {
        return Err(RunnerConfigError::ProviderBackedModesUnavailable);
    }
    if config.policy.allow_tool_execution {
        return Err(RunnerConfigError::ToolExecutionUnavailable);
    }
    validate_automated_repair_policy(config)?;
    Ok(ValidatedRunnerConfig {
        config: config.clone(),
        canonical_private_key_path,
    })
}

pub fn capability_summary(
    validated: &ValidatedRunnerConfig,
) -> Result<CapabilitySummary, RunnerConfigError> {
    let config = &validated.config;
    Ok(CapabilitySummary {
        runner_id: config.runner.runner_id.clone(),
        runner_public_name: config.runner.public_name.clone(),
        capability_schema_version: "1.0".to_owned(),
        capabilities: RedactedCapabilities {
            subjects: config.capabilities.subjects.clone(),
            languages: config.capabilities.languages.clone(),
            phases: config.capabilities.phases.clone(),
            task_types: config.capabilities.task_types.clone(),
            workflow_capabilities: config.capabilities.workflow_capabilities.clone(),
            artifact_types: config.capabilities.artifact_types.clone(),
            tools: config.capabilities.tools.clone(),
            automated_repair_loop_opt_in: config.policy.automated_repair_loop_opt_in,
            automated_repair_attempt_bucket: automated_repair_attempt_bucket(
                config.policy.max_automated_repair_attempts,
            )
            .ok_or(RunnerConfigError::InvalidAutomatedRepairPolicy)?
            .to_owned(),
        },
        trust_level: "runner_candidate".to_owned(),
        policy_summary: RedactedPolicySummary {
            max_tasks_per_day_bucket: config.policy.max_tasks_per_day_bucket.clone(),
            auto_submit_status_cap: config.policy.auto_submit_status_cap.clone(),
            allowed_risk_level_max: config.policy.allowed_risk_level_max.clone(),
        },
    })
}

pub fn automated_repair_attempt_bucket(attempts: u8) -> Option<&'static str> {
    match attempts {
        0 => Some("0"),
        1 => Some("1"),
        2 => Some("2"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DummyPlanningContext {
    pub request_id: String,
    pub planning_task_id: String,
}

impl DummyPlanningContext {
    pub fn mvp_fixture() -> Self {
        Self {
            request_id: "req_energy_001".to_owned(),
            planning_task_id: "ptask_energy_001".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DummyVerificationContext {
    pub proposal_id: String,
    pub verification_task_id: String,
}

impl DummyVerificationContext {
    pub fn mvp_fixture() -> Self {
        Self {
            proposal_id: "plan_energy_001_a".to_owned(),
            verification_task_id: "pvtask_energy_001_a".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DummyGenerationContext {
    pub artifact_id: String,
    pub artifact_intake_ref: String,
    pub request_id: String,
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub execution_policy: String,
    pub claim_workspace_root: PathBuf,
}

impl DummyGenerationContext {
    pub fn mvp_fixture() -> Self {
        Self::mvp_fixture_for_workspace(Path::new(".lessonforge-runner-work"))
    }

    pub fn mvp_fixture_for_workspace(workspace_root: &Path) -> Self {
        let lease_id = "lease_generation_energy_001".to_owned();
        Self {
            artifact_id: "art_energy_001".to_owned(),
            artifact_intake_ref: "aintake_energy_001".to_owned(),
            request_id: "req_energy_001".to_owned(),
            work_packet_id: "wp_energy_001_generate_pack".to_owned(),
            lease_id: lease_id.clone(),
            runner_actor_id: "actor_generator_001".to_owned(),
            execution_policy: "sandboxed_self_test_python_checker".to_owned(),
            claim_workspace_root: workspace_root.join("claims").join(lease_id),
        }
    }

    pub fn output_dir(&self) -> PathBuf {
        self.claim_workspace_root.join("output")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationOutput {
    pub kind: String,
    pub artifact_bundle_reference: ArtifactBundleReference,
    pub provenance: GenerationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactBundleReference {
    pub kind: String,
    pub artifact_intake_ref: String,
    pub manifest_summary: ManifestSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestSummary {
    pub artifact_id: String,
    pub title: String,
    pub subject: String,
    pub topic: String,
    pub age_range: String,
    pub language: String,
    pub license: String,
    pub ai_assisted: bool,
    pub contents: Vec<String>,
    pub known_limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationProvenance {
    pub file_digests: BTreeMap<String, String>,
    pub bundle_digest: String,
    pub runner_self_test_report: RunnerSelfTestReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerFileDigest {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerSelfTestReport {
    pub self_test_report_id: String,
    pub work_packet_id: String,
    pub lease_id: String,
    pub runner_actor_id: String,
    pub execution_policy: String,
    pub execution_profile_id: String,
    pub allowed_command_id: String,
    pub sandbox_enforced: bool,
    pub self_test_status: String,
    pub file_digests: BTreeMap<String, String>,
    pub bundle_digest: String,
    pub checks: Vec<RunnerSelfTestCheck>,
    pub attestation: RunnerSelfTestAttestation,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerSelfTestCheck {
    pub check: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_reason_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerSelfTestAttestation {
    pub attestation_schema_version: String,
    pub signature_kind: String,
    pub runner_key_id: String,
    pub signed_payload_digest: String,
    pub signature: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RunnerOutputError {
    #[error("runner mode mismatch")]
    RunnerModeMismatch,
    #[error("unsafe workspace path")]
    UnsafeWorkspacePath,
    #[error("runner output unavailable")]
    OutputUnavailable,
}

impl RunnerOutputError {
    pub fn safe_code(&self) -> &'static str {
        match self {
            Self::RunnerModeMismatch => "runner_mode_mismatch",
            Self::UnsafeWorkspacePath => "unsafe_workspace_path",
            Self::OutputUnavailable => "runner_output_unavailable",
        }
    }
}

pub fn dummy_request_moderation_report(
    validated: &ValidatedRunnerConfig,
) -> Result<Value, RunnerOutputError> {
    require_mode(validated, RunnerMode::DummyRequestModerator)?;
    Ok(json!({
        "request_moderation_report_id": "rmreport_energy_001",
        "request_moderation_task_id": "rmtask_energy_001",
        "request_id": "req_energy_001",
        "lease_id": "lease_rmoderation_energy_001",
        "moderation_kind": "dummy_fixture",
        "decision": "allow_mvp_planning",
        "category_flags": ["none"],
        "safe_reason_codes": ["moderation_allowed"]
    }))
}

pub fn dummy_proposed_task_graph(
    validated: &ValidatedRunnerConfig,
    context: &DummyPlanningContext,
) -> Result<Value, RunnerOutputError> {
    require_mode(validated, RunnerMode::DummyPlanner)?;
    Ok(json!({
        "proposal_id": "plan_energy_001_a",
        "request_id": context.request_id,
        "planning_task_id": context.planning_task_id,
        "planner_runner_id": validated.config.runner.runner_id,
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
                "execution_policy": "code_generation_only",
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
    }))
}

pub fn dummy_plan_verification(
    validated: &ValidatedRunnerConfig,
    context: &DummyVerificationContext,
) -> Result<Value, RunnerOutputError> {
    require_mode(validated, RunnerMode::DummyPlanVerifier)?;
    Ok(json!({
        "verification_id": "pverify_energy_001_a",
        "proposal_id": context.proposal_id,
        "verification_task_id": context.verification_task_id,
        "verifier_runner_id": validated.config.runner.runner_id,
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
    }))
}

pub fn write_dummy_artifact_bundle(
    validated: &ValidatedRunnerConfig,
    context: &DummyGenerationContext,
    output_root: &Path,
) -> Result<GenerationOutput, RunnerOutputError> {
    require_mode(validated, RunnerMode::DummyGenerator)?;
    let expected_output_root = expected_claim_output_root(validated, context)?;
    if output_root != expected_output_root {
        return Err(RunnerOutputError::UnsafeWorkspacePath);
    }
    let workspace_root = Path::new(&validated.config.runner.workspace_root);
    reject_existing_symlink_workspace_descendants(workspace_root, output_root)?;

    let staging_root = output_root.with_extension("tmp");
    reject_existing_symlink_workspace_descendants(workspace_root, &staging_root)?;
    if output_root.exists() || staging_root.exists() {
        return Err(RunnerOutputError::OutputUnavailable);
    }

    let staged = stage_dummy_artifact_bundle(validated, context, &staging_root);
    let staged = match staged {
        Ok(staged) => staged,
        Err(error) => {
            cleanup_staging_dir(&staging_root);
            return Err(error);
        }
    };
    if fs::rename(&staging_root, output_root).is_err() {
        cleanup_staging_dir(&staging_root);
        return Err(RunnerOutputError::OutputUnavailable);
    }
    reject_existing_symlink_workspace_descendants(workspace_root, output_root)?;
    Ok(GenerationOutput {
        kind: "generation_output_v1".to_owned(),
        artifact_bundle_reference: ArtifactBundleReference {
            kind: "artifact_bundle_reference".to_owned(),
            artifact_intake_ref: context.artifact_intake_ref.clone(),
            manifest_summary: ManifestSummary {
                artifact_id: context.artifact_id.clone(),
                title: "Conservation of energy lesson pack".to_owned(),
                subject: "physics".to_owned(),
                topic: "conservation_of_energy".to_owned(),
                age_range: "14-16".to_owned(),
                language: "en".to_owned(),
                license: "CC-BY-4.0".to_owned(),
                ai_assisted: true,
                contents: vec![
                    "manifest.json".to_owned(),
                    "worksheet.md".to_owned(),
                    "answer_key.md".to_owned(),
                    "checker.py".to_owned(),
                    "teacher_notes.md".to_owned(),
                ],
                known_limitations: vec![
                    "Curriculum standard alignment requires teacher review.".to_owned(),
                ],
            },
        },
        provenance: GenerationProvenance {
            file_digests: staged.file_digest_map,
            bundle_digest: staged.bundle_digest,
            runner_self_test_report: staged.runner_self_test_report,
        },
    })
}

struct StagedDummyArtifactBundle {
    bundle_digest: String,
    file_digest_map: BTreeMap<String, String>,
    runner_self_test_report: RunnerSelfTestReport,
}

fn stage_dummy_artifact_bundle(
    validated: &ValidatedRunnerConfig,
    context: &DummyGenerationContext,
    staging_root: &Path,
) -> Result<StagedDummyArtifactBundle, RunnerOutputError> {
    let workspace_root = Path::new(&validated.config.runner.workspace_root);
    fs::create_dir_all(staging_root).map_err(|_| RunnerOutputError::OutputUnavailable)?;
    reject_existing_symlink_workspace_descendants(workspace_root, staging_root)?;
    for (name, bytes) in dummy_bundle_files(context)? {
        let relative_path = Path::new(name);
        reject_unsafe_path_components(relative_path)?;
        let path = staging_root.join(relative_path);
        reject_existing_symlink_workspace_descendants(workspace_root, &path)?;
        write_new_file_without_following_symlinks(&path, &bytes)?;
    }
    let file_digests = compute_runner_file_digests(staging_root)?;
    let bundle_digest = compute_runner_bundle_digest(&file_digests, context)?;
    let file_digest_map = runner_digest_map(&file_digests);
    let runner_self_test_report = sandbox_unavailable_self_test_report(
        validated,
        context,
        file_digest_map.clone(),
        bundle_digest.clone(),
    )?;
    Ok(StagedDummyArtifactBundle {
        bundle_digest,
        file_digest_map,
        runner_self_test_report,
    })
}

fn cleanup_staging_dir(staging_root: &Path) {
    let _ = fs::remove_dir_all(staging_root);
}

fn write_new_file_without_following_symlinks(
    path: &Path,
    bytes: &[u8],
) -> Result<(), RunnerOutputError> {
    #[cfg(not(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        let _ = path;
        let _ = bytes;
        Err(RunnerOutputError::OutputUnavailable)
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        // Enforce the no-symlink property at open time. Platforms without an
        // equivalent fail closed above instead of relying on pre-open checks.
        options.custom_flags(libc::O_NOFOLLOW);
        let mut file = options
            .open(path)
            .map_err(|_| RunnerOutputError::OutputUnavailable)?;
        file.write_all(bytes)
            .map_err(|_| RunnerOutputError::OutputUnavailable)
    }
}

fn expected_claim_output_root(
    validated: &ValidatedRunnerConfig,
    context: &DummyGenerationContext,
) -> Result<PathBuf, RunnerOutputError> {
    let workspace_root = Path::new(&validated.config.runner.workspace_root);
    reject_unsafe_local_workspace_path(workspace_root)?;
    reject_unsafe_workspace_component(&context.lease_id)?;
    let claim_root = workspace_root.join("claims").join(&context.lease_id);
    if context.claim_workspace_root != claim_root {
        return Err(RunnerOutputError::UnsafeWorkspacePath);
    }
    Ok(claim_root.join("output"))
}

fn reject_unsafe_path_components(path: &Path) -> Result<(), RunnerOutputError> {
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(RunnerOutputError::UnsafeWorkspacePath);
    }
    Ok(())
}

fn reject_unsafe_local_workspace_path(path: &Path) -> Result<(), RunnerOutputError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        || has_non_absolute_prefix(path)
    {
        return Err(RunnerOutputError::UnsafeWorkspacePath);
    }
    Ok(())
}

fn reject_unsafe_workspace_component(value: &str) -> Result<(), RunnerOutputError> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(RunnerOutputError::UnsafeWorkspacePath);
    }
    Ok(())
}

fn reject_existing_symlink_workspace_descendants(
    workspace_root: &Path,
    target: &Path,
) -> Result<(), RunnerOutputError> {
    if fs::symlink_metadata(workspace_root)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(RunnerOutputError::UnsafeWorkspacePath);
    }
    let relative = target
        .strip_prefix(workspace_root)
        .map_err(|_| RunnerOutputError::UnsafeWorkspacePath)?;
    let mut current = workspace_root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(RunnerOutputError::UnsafeWorkspacePath);
        }
    }
    Ok(())
}

fn require_mode(
    validated: &ValidatedRunnerConfig,
    expected: RunnerMode,
) -> Result<(), RunnerOutputError> {
    if validated.config.runner.mode == expected {
        Ok(())
    } else {
        Err(RunnerOutputError::RunnerModeMismatch)
    }
}

fn dummy_bundle_files(
    context: &DummyGenerationContext,
) -> Result<Vec<(&'static str, Vec<u8>)>, RunnerOutputError> {
    let manifest = serde_json::to_vec_pretty(&json!({
        "artifact_id": context.artifact_id,
        "request_id": context.request_id,
        "work_packet_ids": [context.work_packet_id],
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
    }))
    .map_err(|_| RunnerOutputError::OutputUnavailable)?;
    Ok(vec![
        ("manifest.json", manifest),
        ("worksheet.md", WORKSHEET.as_bytes().to_vec()),
        ("answer_key.md", ANSWER_KEY.as_bytes().to_vec()),
        ("checker.py", CHECKER.as_bytes().to_vec()),
        ("teacher_notes.md", TEACHER_NOTES.as_bytes().to_vec()),
    ])
}

const WORKSHEET: &str = r#"# Conservation of energy lesson pack

## Ramp problem

A 2 kg cart starts from rest at a height of 5 m on a frictionless ramp.
Use gravitational potential energy and kinetic energy to estimate its speed at the bottom.

## Extension

Explain how the answer changes if the cart starts halfway down the same ramp.
"#;

const ANSWER_KEY: &str = r#"# Answer key

Gravitational potential energy at the top is m g h.
For a 2 kg cart at 5 m, this is about 98 J.
At the bottom, kinetic energy is one half m v squared, so the speed is about 9.9 m per second.

The halfway starting point has half the height, so the final kinetic energy is half as large.
"#;

const TEACHER_NOTES: &str = r#"# Teacher notes

This draft supports a 45 minute lesson for learners aged 14 to 16.
Keep the focus on energy transfer and avoid calculus.
Ask learners to identify the system before substituting values.
"#;

const CHECKER: &str = r#"import math
def kinetic_energy(mass_kg, speed_m_per_s):
    return 0.5 * mass_kg * speed_m_per_s * speed_m_per_s
def gravitational_potential_energy(mass_kg, g_m_per_s2, height_m):
    return mass_kg * g_m_per_s2 * height_m
def speed_from_kinetic_energy(kinetic_energy_j, mass_kg):
    return math.sqrt((2.0 * kinetic_energy_j) / mass_kg)
"#;

const DIGEST_FILE_ORDER: [&str; 5] = [
    "answer_key.md",
    "checker.py",
    "manifest.json",
    "teacher_notes.md",
    "worksheet.md",
];

fn compute_runner_file_digests(
    output_root: &Path,
) -> Result<Vec<RunnerFileDigest>, RunnerOutputError> {
    let mut digests = Vec::with_capacity(DIGEST_FILE_ORDER.len());
    for path in DIGEST_FILE_ORDER {
        let bytes =
            fs::read(output_root.join(path)).map_err(|_| RunnerOutputError::OutputUnavailable)?;
        digests.push(RunnerFileDigest {
            path: path.to_owned(),
            sha256: sha256_hex(&bytes),
            size_bytes: u64::try_from(bytes.len())
                .map_err(|_| RunnerOutputError::OutputUnavailable)?,
        });
    }
    Ok(digests)
}

#[derive(Debug, Serialize)]
struct CanonicalRunnerBundleDigest<'a> {
    artifact_bundle_schema_version: &'static str,
    execution_policy: &'a str,
    file_digests: &'a [RunnerFileDigest],
    runner_actor_id: &'a str,
    work_packet_id: &'a str,
}

fn compute_runner_bundle_digest(
    file_digests: &[RunnerFileDigest],
    context: &DummyGenerationContext,
) -> Result<String, RunnerOutputError> {
    let canonical = CanonicalRunnerBundleDigest {
        artifact_bundle_schema_version: "mvp-artifact-bundle-v1",
        execution_policy: &context.execution_policy,
        file_digests,
        runner_actor_id: &context.runner_actor_id,
        work_packet_id: &context.work_packet_id,
    };
    let bytes = serde_json_canonicalizer::to_vec(&canonical)
        .map_err(|_| RunnerOutputError::OutputUnavailable)?;
    Ok(sha256_hex(&bytes))
}

fn runner_digest_map(file_digests: &[RunnerFileDigest]) -> BTreeMap<String, String> {
    file_digests
        .iter()
        .map(|digest| (digest.path.clone(), digest.sha256.clone()))
        .collect()
}

fn sandbox_unavailable_self_test_report(
    validated: &ValidatedRunnerConfig,
    context: &DummyGenerationContext,
    file_digests: BTreeMap<String, String>,
    bundle_digest: String,
) -> Result<RunnerSelfTestReport, RunnerOutputError> {
    let (self_test_report_id, created_at) = self_test_report_provenance(context);
    let mut report = RunnerSelfTestReport {
        self_test_report_id,
        work_packet_id: context.work_packet_id.clone(),
        lease_id: context.lease_id.clone(),
        runner_actor_id: context.runner_actor_id.clone(),
        execution_policy: context.execution_policy.clone(),
        execution_profile_id: "python_checker_self_test_v1".to_owned(),
        allowed_command_id: "python_checker_self_test_harness_v1".to_owned(),
        sandbox_enforced: false,
        self_test_status: "not_run_sandbox_unavailable".to_owned(),
        file_digests,
        bundle_digest,
        checks: sandbox_unavailable_checks(),
        attestation: RunnerSelfTestAttestation {
            attestation_schema_version: "runner-self-test-attestation-v1".to_owned(),
            signature_kind: "ed25519".to_owned(),
            runner_key_id: validated.config.attestation.runner_key_id.clone(),
            signed_payload_digest:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            signature: String::new(),
        },
        created_at,
    };
    seal_self_test_report(validated, &mut report)?;
    Ok(report)
}

fn self_test_report_provenance(context: &DummyGenerationContext) -> (String, String) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let counter = SELF_TEST_REPORT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let created_at = rfc3339_utc_from_unix_seconds(now.as_secs());
    let mut hasher = Sha256::new();
    hasher.update(context.work_packet_id.as_bytes());
    hasher.update(context.lease_id.as_bytes());
    hasher.update(context.runner_actor_id.as_bytes());
    hasher.update(now.as_secs().to_be_bytes());
    hasher.update(now.subsec_nanos().to_be_bytes());
    hasher.update(counter.to_be_bytes());
    let digest = hasher.finalize();
    let mut id_bytes = [0_u8; 8];
    id_bytes.copy_from_slice(&digest[..8]);
    (
        format!("rselftest_{:016x}", u64::from_be_bytes(id_bytes)),
        created_at,
    )
}

fn rfc3339_utc_from_unix_seconds(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_date_from_unix_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_date_from_unix_days(days: i64) -> (i64, u32, u32) {
    let shifted_days = days + 719_468;
    let era = if shifted_days >= 0 {
        shifted_days
    } else {
        shifted_days - 146_096
    } / 146_097;
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month as u32, day as u32)
}

fn sandbox_unavailable_checks() -> Vec<RunnerSelfTestCheck> {
    [
        "sandbox_profile_enforced",
        "checker_static_safety",
        "checker_function_contract",
        "checker_sample_cases",
        "no_network_observed",
        "no_filesystem_escape_observed",
        "no_secret_env_present",
    ]
    .into_iter()
    .map(|check| RunnerSelfTestCheck {
        check: check.to_owned(),
        status: "not_run".to_owned(),
        safe_reason_code: Some("sandbox_unavailable".to_owned()),
    })
    .chain(std::iter::once(RunnerSelfTestCheck {
        check: "digest_computed".to_owned(),
        status: "passed".to_owned(),
        safe_reason_code: None,
    }))
    .collect()
}

fn seal_self_test_report(
    validated: &ValidatedRunnerConfig,
    report: &mut RunnerSelfTestReport,
) -> Result<(), RunnerOutputError> {
    let payload = self_test_attestation_payload_canonical_bytes(report)?;
    report.attestation.signed_payload_digest = sha256_hex(&payload);
    let signing_key = signing_key_from_config(validated)?;
    let signature = signing_key.sign(&payload);
    report.attestation.signature =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes());
    Ok(())
}

fn signing_key_from_config(
    validated: &ValidatedRunnerConfig,
) -> Result<SigningKey, RunnerOutputError> {
    let path = validated
        .canonical_private_key_path
        .as_deref()
        .ok_or(RunnerOutputError::OutputUnavailable)?;
    let bytes =
        read_bounded_ed25519_key_file(path).map_err(|_| RunnerOutputError::OutputUnavailable)?;
    let seed = parse_ed25519_seed(&bytes).ok_or(RunnerOutputError::OutputUnavailable)?;
    Ok(SigningKey::from_bytes(&seed))
}

fn read_bounded_ed25519_key_file(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ed25519 key path is not a regular file",
        ));
    }
    reject_insecure_key_file_permissions(&metadata)?;
    let mut file = open_key_file_without_following_symlinks(path)?;
    let opened_metadata = file.metadata()?;
    if !opened_metadata.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ed25519 key path is not a regular file",
        ));
    }
    reject_insecure_key_file_permissions(&opened_metadata)?;
    if opened_metadata.len() > MAX_ED25519_KEY_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ed25519 key file too large",
        ));
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(MAX_ED25519_KEY_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ED25519_KEY_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "ed25519 key file too large",
        ));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn reject_insecure_key_file_permissions(metadata: &fs::Metadata) -> Result<(), std::io::Error> {
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "ed25519 key file has insecure permissions",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_insecure_key_file_permissions(_metadata: &fs::Metadata) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn open_key_file_without_following_symlinks(path: &Path) -> Result<fs::File, std::io::Error> {
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(target_os = "windows")]
fn open_key_file_without_following_symlinks(path: &Path) -> Result<fs::File, std::io::Error> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "windows"
)))]
fn open_key_file_without_following_symlinks(_path: &Path) -> Result<fs::File, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "secure key file open is unsupported on this target",
    ))
}

fn parse_ed25519_seed(bytes: &[u8]) -> Option<[u8; 32]> {
    if bytes.len() == 32 {
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(bytes);
        return Some(seed);
    }
    let text = std::str::from_utf8(bytes).ok()?.trim();
    if text.len() != 64 {
        return None;
    }
    let mut seed = [0_u8; 32];
    for (index, chunk) in text.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        seed[index] = (high << 4) | low;
    }
    Some(seed)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
struct SelfTestAttestationPayload<'a> {
    attestation_schema_version: &'a str,
    runner_actor_id: &'a str,
    runner_key_id: &'a str,
    signature_kind: &'a str,
    self_test_report_id: &'a str,
    work_packet_id: &'a str,
    lease_id: &'a str,
    execution_policy: &'a str,
    execution_profile_id: &'a str,
    allowed_command_id: &'a str,
    sandbox_enforced: bool,
    bundle_digest: &'a str,
    file_digests: &'a BTreeMap<String, String>,
    checks: &'a [RunnerSelfTestCheck],
    self_test_status: &'a str,
    created_at: &'a str,
}

fn self_test_attestation_payload_canonical_bytes(
    report: &RunnerSelfTestReport,
) -> Result<Vec<u8>, RunnerOutputError> {
    let payload = SelfTestAttestationPayload {
        attestation_schema_version: &report.attestation.attestation_schema_version,
        runner_actor_id: &report.runner_actor_id,
        runner_key_id: &report.attestation.runner_key_id,
        signature_kind: &report.attestation.signature_kind,
        self_test_report_id: &report.self_test_report_id,
        work_packet_id: &report.work_packet_id,
        lease_id: &report.lease_id,
        execution_policy: &report.execution_policy,
        execution_profile_id: &report.execution_profile_id,
        allowed_command_id: &report.allowed_command_id,
        sandbox_enforced: report.sandbox_enforced,
        bundle_digest: &report.bundle_digest,
        file_digests: &report.file_digests,
        checks: &report.checks,
        self_test_status: &report.self_test_status,
        created_at: &report.created_at,
    };
    serde_json_canonicalizer::to_vec(&payload).map_err(|_| RunnerOutputError::OutputUnavailable)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn validate_private_fields(config: &RunnerConfig) -> Result<(), RunnerConfigError> {
    let fields = &config.forbidden;
    if fields.provider_api_key_literal.is_some()
        || fields.provider_base_url.is_some()
        || fields.auth_path.is_some()
        || fields.cookie_path.is_some()
        || fields.browser_profile_path.is_some()
        || fields.local_prompt_template.is_some()
        || fields.exact_quota.is_some()
        || fields.hidden_network_destination.is_some()
    {
        return Err(RunnerConfigError::UnsafePrivateConfigField);
    }
    Ok(())
}

fn validate_origin(origin: &str, tls: &TlsSection) -> Result<(), RunnerConfigError> {
    let (scheme, rest) = origin
        .split_once("://")
        .ok_or(RunnerConfigError::UnsafeCentralApiOrigin)?;
    if rest.is_empty()
        || rest.contains('@')
        || rest.contains('/')
        || rest.contains('?')
        || rest.contains('#')
    {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    let host = parse_host(rest)?;
    if host.is_empty() || host.contains('*') {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    let loopback = matches!(host, "127.0.0.1" | "::1" | "localhost");
    if scheme == "http" && !loopback {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    let lower = origin.to_ascii_lowercase();
    let provider_default_port = parse_port(rest)
        .transpose()?
        .is_some_and(|port| port == 11_434);
    if lower.contains("ollama")
        || lower.contains("openai")
        || lower.contains("anthropic")
        || provider_default_port
    {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    if scheme != "https" && !(scheme == "http" && loopback) {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    match tls.trust_policy {
        TlsTrustPolicy::LoopbackDevelopment => {
            if !loopback
                || scheme != "http"
                || !tls.pinned_ca_pem_path.is_empty()
                || !tls.pinned_spki_sha256.is_empty()
                || !tls.expected_server_name.is_empty()
            {
                return Err(RunnerConfigError::UnsafeCentralApiOrigin);
            }
        }
        TlsTrustPolicy::PinnedCa => {
            if scheme != "https" {
                return Err(RunnerConfigError::UnsafeCentralApiOrigin);
            }
            validate_pin_path(&tls.pinned_ca_pem_path)?;
            validate_expected_server_name(&tls.expected_server_name, host)?;
            if !tls.pinned_spki_sha256.is_empty() {
                return Err(RunnerConfigError::UnsafeCentralApiOrigin);
            }
        }
        TlsTrustPolicy::PinnedSpki => {
            if scheme != "https" {
                return Err(RunnerConfigError::UnsafeCentralApiOrigin);
            }
            validate_spki_pin(&tls.pinned_spki_sha256)?;
            validate_expected_server_name(&tls.expected_server_name, host)?;
            if !tls.pinned_ca_pem_path.is_empty() {
                return Err(RunnerConfigError::UnsafeCentralApiOrigin);
            }
        }
    }
    Ok(())
}

fn validate_pin_path(path: &str) -> Result<(), RunnerConfigError> {
    if path.is_empty()
        || path.contains("..")
        || path.contains('\0')
        || path.contains("~")
        || !path
            .chars()
            .all(|character| character.is_ascii() && !character.is_control())
    {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    let path = Path::new(path);
    if path.file_name().is_none_or(|name| {
        name.to_str()
            .is_none_or(|name| name.is_empty() || !name.ends_with(".pem"))
    }) {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| RunnerConfigError::UnsafeCentralApiOrigin)?;
    if !metadata.file_type().is_file() {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    let file = open_pin_file_without_following_symlinks(path)?;
    let opened_metadata = file
        .metadata()
        .map_err(|_| RunnerConfigError::UnsafeCentralApiOrigin)?;
    if !opened_metadata.file_type().is_file() {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    Ok(())
}

fn open_pin_file_without_following_symlinks(path: &Path) -> Result<fs::File, RunnerConfigError> {
    #[cfg(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        let mut options = OpenOptions::new();
        options.read(true);
        options.custom_flags(libc::O_NOFOLLOW);
        options
            .open(path)
            .map_err(|_| RunnerConfigError::UnsafeCentralApiOrigin)
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| RunnerConfigError::UnsafeCentralApiOrigin)
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "windows"
    )))]
    {
        let _ = path;
        Err(RunnerConfigError::UnsafeCentralApiOrigin)
    }
}

fn validate_spki_pin(value: &str) -> Result<(), RunnerConfigError> {
    let is_lower_hex_sha256 = value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'));
    if is_lower_hex_sha256 {
        Ok(())
    } else {
        Err(RunnerConfigError::UnsafeCentralApiOrigin)
    }
}

fn validate_expected_server_name(expected: &str, host: &str) -> Result<(), RunnerConfigError> {
    if expected == host
        && !expected.contains('*')
        && !expected.is_empty()
        && expected
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
    {
        Ok(())
    } else {
        Err(RunnerConfigError::UnsafeCentralApiOrigin)
    }
}

fn parse_host(rest: &str) -> Result<&str, RunnerConfigError> {
    if let Some(bracketed) = rest.strip_prefix('[') {
        let Some((host, port_part)) = bracketed.split_once(']') else {
            return Err(RunnerConfigError::UnsafeCentralApiOrigin);
        };
        if !port_part.is_empty() && !(port_part.strip_prefix(':').is_some_and(is_decimal_port)) {
            return Err(RunnerConfigError::UnsafeCentralApiOrigin);
        }
        return Ok(host);
    }
    let mut parts = rest.split(':');
    let host = parts.next().unwrap_or_default();
    if let Some(port) = parts.next()
        && !is_decimal_port(port)
    {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    if parts.next().is_some() {
        return Err(RunnerConfigError::UnsafeCentralApiOrigin);
    }
    Ok(host)
}

fn is_decimal_port(port: &str) -> bool {
    !port.is_empty() && port.chars().all(|character| character.is_ascii_digit())
}

fn parse_port(rest: &str) -> Option<Result<u16, RunnerConfigError>> {
    let port = if let Some(bracketed) = rest.strip_prefix('[') {
        let (_, port_part) = bracketed.split_once(']')?;
        if port_part.is_empty() {
            return None;
        }
        port_part.strip_prefix(':')?
    } else {
        let mut parts = rest.split(':');
        let _host = parts.next()?;
        let port = parts.next()?;
        if parts.next().is_some() {
            return Some(Err(RunnerConfigError::UnsafeCentralApiOrigin));
        }
        port
    };
    Some(
        port.parse::<u16>()
            .map_err(|_| RunnerConfigError::UnsafeCentralApiOrigin),
    )
}

fn validate_summary_fields(config: &RunnerConfig) -> Result<(), RunnerConfigError> {
    for value in [
        config.runner.runner_id.as_str(),
        config.runner.public_name.as_str(),
        config.runner.scope_id.as_str(),
        config.policy.max_tasks_per_day_bucket.as_str(),
        config.policy.auto_submit_status_cap.as_str(),
        config.policy.allowed_risk_level_max.as_str(),
    ] {
        validate_central_safe_summary_text(value)?;
    }
    for value in config
        .capabilities
        .subjects
        .iter()
        .chain(config.capabilities.languages.iter())
        .chain(config.capabilities.phases.iter())
        .chain(config.capabilities.task_types.iter())
        .chain(config.capabilities.workflow_capabilities.iter())
        .chain(config.capabilities.artifact_types.iter())
        .chain(config.capabilities.tools.iter())
    {
        validate_central_safe_summary_text(value)?;
    }
    Ok(())
}

fn validate_central_safe_summary_text(value: &str) -> Result<(), RunnerConfigError> {
    let lower = value.to_ascii_lowercase();
    let unsafe_marker = lower.contains("sk-")
        || lower.contains("api_key")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("cookie")
        || lower.contains("provider")
        || lower.contains("prompt")
        || lower.contains("quota")
        || lower.contains("account")
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("/users/")
        || lower.contains("/etc/")
        || lower.contains("/private/")
        || lower.contains("/tmp/")
        || lower.contains("\\users\\")
        || lower.contains(".ssh");
    if value.is_empty()
        || value.chars().count() > 96
        || unsafe_marker
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ' ')
        })
    {
        return Err(RunnerConfigError::UnsafeSummaryField);
    }
    Ok(())
}

fn validate_capabilities_match_mode(config: &RunnerConfig) -> Result<(), RunnerConfigError> {
    if config.capabilities == CapabilityConfig::for_mode(config.runner.mode) {
        Ok(())
    } else {
        Err(RunnerConfigError::CapabilityModeMismatch)
    }
}

fn validate_attestation_config(
    config: &RunnerConfig,
) -> Result<Option<PathBuf>, RunnerConfigError> {
    let requires_attestation = config.runner.mode == RunnerMode::DummyGenerator;
    if !requires_attestation {
        if config.attestation.runner_key_id.is_empty()
            && config.attestation.ed25519_private_key_path.is_empty()
            && config.attestation.runner_private_key_dir.is_empty()
        {
            return Ok(None);
        }
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }

    validate_runner_key_id(&config.attestation.runner_key_id)?;
    let key_path = Path::new(&config.attestation.ed25519_private_key_path);
    reject_unsafe_local_config_path(key_path)?;
    let canonical_key_path = key_path
        .canonicalize()
        .map_err(|_| RunnerConfigError::InvalidAttestationConfig)?;
    let workspace_root = Path::new(&config.runner.workspace_root).canonicalize().ok();
    let key_in_workspace = workspace_root
        .as_ref()
        .is_some_and(|workspace_root| canonical_key_path.starts_with(workspace_root));
    let key_in_private_dir = validate_runner_private_key_dir(
        &config.attestation.runner_private_key_dir,
        &canonical_key_path,
    )?;
    if (!key_in_workspace && !key_in_private_dir)
        || read_bounded_ed25519_key_file(&canonical_key_path)
            .ok()
            .and_then(|bytes| parse_ed25519_seed(&bytes))
            .is_none()
    {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }
    Ok(Some(canonical_key_path))
}

fn validate_runner_private_key_dir(
    configured_dir: &str,
    canonical_key_path: &Path,
) -> Result<bool, RunnerConfigError> {
    if configured_dir.is_empty() {
        return Ok(false);
    }
    let dir = Path::new(configured_dir);
    if !dir.is_absolute() {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }
    reject_unsafe_local_config_path(dir)?;
    let dir_file_type = fs::symlink_metadata(dir)
        .map_err(|_| RunnerConfigError::InvalidAttestationConfig)?
        .file_type();
    if dir_file_type.is_symlink() {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }
    let canonical_dir = dir
        .canonicalize()
        .map_err(|_| RunnerConfigError::InvalidAttestationConfig)?;
    let metadata =
        fs::metadata(&canonical_dir).map_err(|_| RunnerConfigError::InvalidAttestationConfig)?;
    if !metadata.is_dir() {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }
    fs::read_dir(&canonical_dir).map_err(|_| RunnerConfigError::InvalidAttestationConfig)?;
    Ok(canonical_key_path.starts_with(&canonical_dir))
}

fn reject_unsafe_local_config_path(path: &Path) -> Result<(), RunnerConfigError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        || has_non_absolute_prefix(path)
    {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }
    Ok(())
}

#[cfg(windows)]
fn has_non_absolute_prefix(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::Prefix(_)))
        && !path.is_absolute()
}

#[cfg(not(windows))]
fn has_non_absolute_prefix(_path: &Path) -> bool {
    false
}

fn validate_runner_key_id(value: &str) -> Result<(), RunnerConfigError> {
    let Some(suffix) = value.strip_prefix("rkey_") else {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    };
    if suffix.is_empty()
        || suffix.len() > 96
        || !suffix.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(RunnerConfigError::InvalidAttestationConfig);
    }
    Ok(())
}

fn validate_automated_repair_policy(config: &RunnerConfig) -> Result<(), RunnerConfigError> {
    let auto_mode = config.runner.mode == RunnerMode::DummyCodeRepairerAutoLoop;
    let opt_in = config.policy.automated_repair_loop_opt_in;
    let attempts = config.policy.max_automated_repair_attempts;
    let valid = if opt_in {
        auto_mode && matches!(attempts, 1 | 2)
    } else {
        attempts == 0
    };
    if valid {
        Ok(())
    } else {
        Err(RunnerConfigError::InvalidAutomatedRepairPolicy)
    }
}

fn strings(values: impl IntoIterator<Item = &'static str>) -> Vec<String> {
    values.into_iter().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn local_windows_paths_allow_absolute_and_reject_drive_relative_prefixes() {
        assert!(reject_unsafe_local_workspace_path(Path::new(r"C:\lessonforge\work")).is_ok());
        assert!(reject_unsafe_local_workspace_path(Path::new(r"\\server\share\work")).is_ok());
        assert_eq!(
            reject_unsafe_local_workspace_path(Path::new(r"C:lessonforge\work")),
            Err(RunnerOutputError::UnsafeWorkspacePath)
        );
        assert!(
            reject_unsafe_local_config_path(Path::new(r"C:\lessonforge\work\keys\runner.hex"))
                .is_ok()
        );
        assert_eq!(
            reject_unsafe_local_config_path(Path::new(r"C:lessonforge\work\keys\runner.hex")),
            Err(RunnerConfigError::InvalidAttestationConfig)
        );
        assert_eq!(
            reject_unsafe_path_components(Path::new(r"C:\lessonforge\work")),
            Err(RunnerOutputError::UnsafeWorkspacePath)
        );
    }
}
