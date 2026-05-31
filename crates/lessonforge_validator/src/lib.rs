use std::collections::{BTreeSet, HashSet};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const VALIDATOR_NAME: &str = "deterministic_validator";
const ALLOWED_FILES: [&str; 5] = [
    "manifest.json",
    "worksheet.md",
    "answer_key.md",
    "checker.py",
    "teacher_notes.md",
];
const DIGEST_FILE_ORDER: [&str; 5] = [
    "answer_key.md",
    "checker.py",
    "manifest.json",
    "teacher_notes.md",
    "worksheet.md",
];
const MARKDOWN_FILES: [&str; 3] = ["worksheet.md", "answer_key.md", "teacher_notes.md"];
const WHOLE_BUNDLE_MAX_BYTES: u64 = 256 * 1024;

pub fn crate_boundary() -> &'static str {
    VALIDATOR_NAME
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckerExecutionMode {
    StaticOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactValidationContext {
    pub validation_report_id: String,
    pub artifact_id: String,
    pub request_id: String,
    pub generation_work_packet_id: String,
    pub generation_execution_policy: String,
    pub runner_actor_id: String,
    pub validator_version: String,
    pub execution_mode: CheckerExecutionMode,
    pub submitted_digests: Option<SubmittedArtifactDigests>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedArtifactDigests {
    pub file_digests: Vec<FileDigestRecord>,
    pub bundle_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationReportStatus {
    Passed,
    Failed,
    IncompleteStaticOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckStatus {
    Passed,
    Failed,
    SkippedStaticOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationCheckName {
    BundleShape,
    PathNormalization,
    ManifestSchema,
    RequiredFiles,
    FileSizeLimits,
    Utf8Text,
    LicenseMetadata,
    AiAssistanceDisclosure,
    ManifestLineage,
    ContentsMatchFiles,
    MarkdownSafety,
    ObviousPiiHeuristic,
    ObviousInappropriateContentHeuristic,
    SecretLikeValueHeuristic,
    PythonCheckerStaticSafety,
    PythonCheckerRuns,
    NoExternalNetworkStatic,
    PublicProvenanceAllowlist,
}

impl ValidationCheckName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BundleShape => "bundle_shape",
            Self::PathNormalization => "path_normalization",
            Self::ManifestSchema => "manifest_schema",
            Self::RequiredFiles => "required_files",
            Self::FileSizeLimits => "file_size_limits",
            Self::Utf8Text => "utf8_text",
            Self::LicenseMetadata => "license_metadata",
            Self::AiAssistanceDisclosure => "ai_assistance_disclosure",
            Self::ManifestLineage => "manifest_lineage",
            Self::ContentsMatchFiles => "contents_match_files",
            Self::MarkdownSafety => "markdown_safety",
            Self::ObviousPiiHeuristic => "obvious_pii_heuristic",
            Self::ObviousInappropriateContentHeuristic => "obvious_inappropriate_content_heuristic",
            Self::SecretLikeValueHeuristic => "secret_like_value_heuristic",
            Self::PythonCheckerStaticSafety => "python_checker_static_safety",
            Self::PythonCheckerRuns => "python_checker_runs",
            Self::NoExternalNetworkStatic => "no_external_network_static",
            Self::PublicProvenanceAllowlist => "public_provenance_allowlist",
        }
    }
}

const CHECK_ORDER: [ValidationCheckName; 18] = [
    ValidationCheckName::BundleShape,
    ValidationCheckName::PathNormalization,
    ValidationCheckName::ManifestSchema,
    ValidationCheckName::RequiredFiles,
    ValidationCheckName::FileSizeLimits,
    ValidationCheckName::Utf8Text,
    ValidationCheckName::LicenseMetadata,
    ValidationCheckName::AiAssistanceDisclosure,
    ValidationCheckName::ManifestLineage,
    ValidationCheckName::ContentsMatchFiles,
    ValidationCheckName::MarkdownSafety,
    ValidationCheckName::ObviousPiiHeuristic,
    ValidationCheckName::ObviousInappropriateContentHeuristic,
    ValidationCheckName::SecretLikeValueHeuristic,
    ValidationCheckName::PythonCheckerStaticSafety,
    ValidationCheckName::PythonCheckerRuns,
    ValidationCheckName::NoExternalNetworkStatic,
    ValidationCheckName::PublicProvenanceAllowlist,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationCheckRecord {
    pub check: ValidationCheckName,
    pub status: CheckStatus,
    pub safe_message: String,
    pub safe_location: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactValidationReport {
    pub validation_report_id: String,
    pub artifact_id: String,
    pub validator: String,
    pub validator_version: String,
    pub status: ValidationReportStatus,
    pub checks: Vec<ValidationCheckRecord>,
    pub failures: Vec<ValidationCheckRecord>,
    pub authoritative_digests: ArtifactDigestSet,
}

impl ArtifactValidationReport {
    pub fn check_status(&self, check: ValidationCheckName) -> Option<CheckStatus> {
        self.checks
            .iter()
            .find(|record| record.check == check)
            .map(|record| record.status)
    }

    pub fn all_checks_are_in_spec_order(&self) -> bool {
        self.checks
            .iter()
            .map(|record| record.check)
            .eq(CHECK_ORDER)
    }

    pub fn opens_machine_validation(&self) -> bool {
        self.status == ValidationReportStatus::Passed
            && self
                .checks
                .iter()
                .all(|record| record.status == CheckStatus::Passed)
    }

    pub fn failure_codes(&self) -> Vec<&str> {
        self.failures
            .iter()
            .map(|record| record.safe_message.as_str())
            .collect()
    }

    pub fn safe_locations_are_allowlisted(&self) -> bool {
        self.checks
            .iter()
            .all(|record| safe_location_is_allowlisted(&record.safe_location))
    }

    pub fn rendered_safe_text(&self) -> String {
        let mut output = String::new();
        output.push_str(&self.validation_report_id);
        output.push_str(&self.artifact_id);
        output.push_str(&self.validator);
        output.push_str(&self.validator_version);
        for record in &self.checks {
            output.push_str(record.check.as_str());
            output.push_str(&record.safe_message);
            output.push_str(&record.safe_location);
        }
        output
    }

    pub fn public_provenance(&self) -> PublicProvenance {
        PublicProvenance {
            artifact_id: self.artifact_id.clone(),
            generated_by_category: "runner_assisted".to_owned(),
            validation_category: VALIDATOR_NAME.to_owned(),
            validation_state_summary: match self.status {
                ValidationReportStatus::Passed => "trusted_passed",
                ValidationReportStatus::Failed | ValidationReportStatus::IncompleteStaticOnly => {
                    "trusted_failed"
                }
            }
            .to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicProvenance {
    pub artifact_id: String,
    pub generated_by_category: String,
    pub validation_category: String,
    pub validation_state_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDigestSet {
    pub file_digests: Vec<FileDigestRecord>,
    pub bundle_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileDigestRecord {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
struct CanonicalBundleDigest<'a> {
    artifact_bundle_schema_version: &'static str,
    execution_policy: &'a str,
    file_digests: &'a [FileDigestRecord],
    runner_actor_id: &'a str,
    work_packet_id: &'a str,
}

#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("bundle path could not be inspected")]
    BundleInaccessible,
    #[error("artifact digest could not be computed")]
    DigestUnavailable,
}

pub fn validate_bundle(
    bundle_root: &Path,
    context: &ArtifactValidationContext,
) -> Result<ArtifactValidationReport, ValidatorError> {
    let mut builder = ReportBuilder::new(context);
    let bundle = inspect_bundle(bundle_root, &mut builder);
    let authoritative_digests = match compute_artifact_digests(bundle_root, context) {
        Ok(digests) => digests,
        Err(_) => {
            builder.fail(
                ValidationCheckName::PublicProvenanceAllowlist,
                "artifact_digest_mismatch",
            );
            ArtifactDigestSet {
                file_digests: Vec::new(),
                bundle_digest:
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_owned(),
            }
        }
    };
    if let Some(submitted) = &context.submitted_digests
        && (submitted.file_digests != authoritative_digests.file_digests
            || submitted.bundle_digest != authoritative_digests.bundle_digest)
    {
        builder.fail(
            ValidationCheckName::PublicProvenanceAllowlist,
            "artifact_digest_mismatch",
        );
    }
    let manifest = validate_manifest(bundle_root, &bundle, context, &mut builder);
    validate_markdown(bundle_root, &bundle, &manifest, &mut builder);
    validate_checker(bundle_root, &bundle, context.execution_mode, &mut builder);
    Ok(builder.finish(authoritative_digests))
}

pub fn compute_artifact_digests(
    bundle_root: &Path,
    context: &ArtifactValidationContext,
) -> Result<ArtifactDigestSet, ValidatorError> {
    let mut file_digests = Vec::with_capacity(DIGEST_FILE_ORDER.len());
    for path in DIGEST_FILE_ORDER {
        let full_path = bundle_root.join(path);
        let metadata =
            fs::symlink_metadata(&full_path).map_err(|_| ValidatorError::DigestUnavailable)?;
        if !metadata.file_type().is_file() {
            return Err(ValidatorError::DigestUnavailable);
        }
        #[cfg(unix)]
        if metadata.nlink() > 1 {
            return Err(ValidatorError::DigestUnavailable);
        }
        let bytes = fs::read(&full_path).map_err(|_| ValidatorError::DigestUnavailable)?;
        file_digests.push(FileDigestRecord {
            path: path.to_owned(),
            sha256: sha256_hex(&bytes),
            size_bytes: u64::try_from(bytes.len())
                .map_err(|_| ValidatorError::DigestUnavailable)?,
        });
    }
    let canonical = CanonicalBundleDigest {
        artifact_bundle_schema_version: "mvp-artifact-bundle-v1",
        execution_policy: &context.generation_execution_policy,
        file_digests: &file_digests,
        runner_actor_id: &context.runner_actor_id,
        work_packet_id: &context.generation_work_packet_id,
    };
    let bytes = serde_jcs::to_vec(&canonical).map_err(|_| ValidatorError::DigestUnavailable)?;
    Ok(ArtifactDigestSet {
        file_digests,
        bundle_digest: sha256_hex(&bytes),
    })
}

#[derive(Debug, Default)]
struct InspectedBundle {
    root_files: BTreeSet<String>,
    text_files: BTreeSet<String>,
    total_size: u64,
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

struct ReportBuilder {
    report: ArtifactValidationReport,
}

impl ReportBuilder {
    fn new(context: &ArtifactValidationContext) -> Self {
        let checks = CHECK_ORDER
            .iter()
            .map(|check| ValidationCheckRecord {
                check: *check,
                status: CheckStatus::Passed,
                safe_message: format!("{}_passed", check.as_str()),
                safe_location: safe_location_for_check(*check).to_owned(),
            })
            .collect();
        Self {
            report: ArtifactValidationReport {
                validation_report_id: context.validation_report_id.clone(),
                artifact_id: context.artifact_id.clone(),
                validator: VALIDATOR_NAME.to_owned(),
                validator_version: context.validator_version.clone(),
                status: ValidationReportStatus::Passed,
                checks,
                failures: Vec::new(),
                authoritative_digests: ArtifactDigestSet {
                    file_digests: Vec::new(),
                    bundle_digest:
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            .to_owned(),
                },
            },
        }
    }

    fn fail(&mut self, check: ValidationCheckName, safe_code: &'static str) {
        self.set(check, CheckStatus::Failed, safe_code);
    }

    fn skip_static_only(&mut self, check: ValidationCheckName, safe_code: &'static str) {
        self.set(check, CheckStatus::SkippedStaticOnly, safe_code);
    }

    fn set(&mut self, check: ValidationCheckName, status: CheckStatus, safe_code: &'static str) {
        if let Some(record) = self
            .report
            .checks
            .iter_mut()
            .find(|record| record.check == check)
        {
            record.status = status;
            record.safe_message = safe_code.to_owned();
            record.safe_location = safe_location_for_check(check).to_owned();
        }
    }

    fn finish(mut self, authoritative_digests: ArtifactDigestSet) -> ArtifactValidationReport {
        self.report.authoritative_digests = authoritative_digests;
        self.report.failures = self
            .report
            .checks
            .iter()
            .filter(|record| record.status != CheckStatus::Passed)
            .cloned()
            .collect();
        self.report.status = if self
            .report
            .checks
            .iter()
            .any(|record| record.status == CheckStatus::Failed)
        {
            ValidationReportStatus::Failed
        } else if self
            .report
            .checks
            .iter()
            .any(|record| record.status == CheckStatus::SkippedStaticOnly)
        {
            ValidationReportStatus::IncompleteStaticOnly
        } else {
            ValidationReportStatus::Passed
        };
        self.report
    }
}

fn inspect_bundle(bundle_root: &Path, builder: &mut ReportBuilder) -> Option<InspectedBundle> {
    let entries = match fs::read_dir(bundle_root) {
        Ok(entries) => entries,
        Err(_) => {
            builder.fail(ValidationCheckName::BundleShape, "bundle_shape_failed");
            return None;
        }
    };

    let mut bundle = InspectedBundle::default();
    let mut normalized_names = HashSet::new();
    for entry in entries {
        let Ok(entry) = entry else {
            builder.fail(ValidationCheckName::BundleShape, "bundle_shape_failed");
            continue;
        };
        let name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => {
                builder.fail(
                    ValidationCheckName::PathNormalization,
                    "path_normalization_failed",
                );
                continue;
            }
        };
        if !path_name_is_safe(&name) || !normalized_names.insert(name.to_ascii_lowercase()) {
            builder.fail(
                ValidationCheckName::PathNormalization,
                "path_normalization_failed",
            );
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            builder.fail(ValidationCheckName::BundleShape, "bundle_shape_failed");
            continue;
        };
        if !metadata.file_type().is_file() {
            builder.fail(ValidationCheckName::BundleShape, "bundle_shape_failed");
            continue;
        }
        #[cfg(unix)]
        if metadata.nlink() > 1 {
            builder.fail(ValidationCheckName::BundleShape, "bundle_shape_failed");
            continue;
        }
        bundle.total_size = bundle.total_size.saturating_add(metadata.len());
        if metadata.len() > max_size_for_file(&name).unwrap_or(0)
            || bundle.total_size > WHOLE_BUNDLE_MAX_BYTES
        {
            builder.fail(
                ValidationCheckName::FileSizeLimits,
                "file_size_limits_failed",
            );
        }
        if !ALLOWED_FILES.contains(&name.as_str()) {
            builder.fail(ValidationCheckName::BundleShape, "bundle_shape_failed");
            continue;
        }
        bundle.root_files.insert(name.clone());
        if read_utf8_file(bundle_root, &name).is_some() {
            bundle.text_files.insert(name);
        } else {
            builder.fail(ValidationCheckName::Utf8Text, "utf8_text_failed");
        }
    }

    for required in ALLOWED_FILES {
        if !bundle.root_files.contains(required) {
            builder.fail(ValidationCheckName::RequiredFiles, "required_files_failed");
        }
    }
    Some(bundle)
}

fn validate_manifest(
    bundle_root: &Path,
    bundle: &Option<InspectedBundle>,
    context: &ArtifactValidationContext,
    builder: &mut ReportBuilder,
) -> Option<ArtifactManifest> {
    if bundle
        .as_ref()
        .is_none_or(|bundle| !bundle.root_files.contains("manifest.json"))
    {
        builder.fail(
            ValidationCheckName::ManifestSchema,
            "manifest_schema_failed",
        );
        return None;
    }
    let Some(text) = read_utf8_file(bundle_root, "manifest.json") else {
        builder.fail(
            ValidationCheckName::ManifestSchema,
            "manifest_schema_failed",
        );
        return None;
    };
    let Ok(manifest) = serde_json::from_str::<ArtifactManifest>(&text) else {
        builder.fail(
            ValidationCheckName::ManifestSchema,
            "manifest_schema_failed",
        );
        return None;
    };

    if !manifest_schema_values_are_valid(&manifest) {
        builder.fail(
            ValidationCheckName::ManifestSchema,
            "manifest_schema_failed",
        );
    }
    if manifest.license != "CC-BY-4.0" {
        builder.fail(
            ValidationCheckName::LicenseMetadata,
            "license_metadata_failed",
        );
    }
    if !manifest.ai_assisted {
        builder.fail(
            ValidationCheckName::AiAssistanceDisclosure,
            "ai_assistance_disclosure_failed",
        );
    }
    if manifest.artifact_id != context.artifact_id
        || manifest.request_id != context.request_id
        || manifest.work_packet_ids != [context.generation_work_packet_id.clone()]
    {
        builder.fail(
            ValidationCheckName::ManifestLineage,
            "artifact_lineage_mismatch",
        );
    }
    if let Some(bundle) = bundle {
        let expected: Vec<String> = ALLOWED_FILES
            .iter()
            .map(|file| (*file).to_owned())
            .collect();
        if manifest.contents != expected
            || bundle.root_files != ALLOWED_FILES.into_iter().map(str::to_owned).collect()
        {
            builder.fail(
                ValidationCheckName::ContentsMatchFiles,
                "contents_match_files_failed",
            );
        }
    }
    if unsafe_public_text(&manifest.title)
        || manifest
            .known_limitations
            .iter()
            .any(|value| unsafe_public_text(value))
    {
        builder.fail(
            ValidationCheckName::SecretLikeValueHeuristic,
            "secret_like_value_heuristic_failed",
        );
    }
    Some(manifest)
}

fn validate_markdown(
    bundle_root: &Path,
    bundle: &Option<InspectedBundle>,
    manifest: &Option<ArtifactManifest>,
    builder: &mut ReportBuilder,
) {
    if bundle.is_none() {
        return;
    }
    let mut pii_failed = false;
    let mut inappropriate_failed = false;
    let mut secret_failed = false;
    for file in MARKDOWN_FILES {
        let Some(text) = read_utf8_file(bundle_root, file) else {
            continue;
        };
        if unsafe_markdown(&text) {
            builder.fail(
                ValidationCheckName::MarkdownSafety,
                "markdown_safety_failed",
            );
        }
        if pii_like(&text) {
            pii_failed = true;
        }
        if inappropriate_content(&text) {
            inappropriate_failed = true;
        }
        if secret_like(&text) {
            secret_failed = true;
        }
    }
    if let Some(manifest) = manifest {
        for value in std::iter::once(&manifest.title).chain(manifest.known_limitations.iter()) {
            if pii_like(value) {
                pii_failed = true;
            }
            if inappropriate_content(value) {
                inappropriate_failed = true;
            }
            if secret_like(value) {
                secret_failed = true;
            }
        }
    }
    if pii_failed {
        builder.fail(
            ValidationCheckName::ObviousPiiHeuristic,
            "obvious_pii_heuristic_failed",
        );
    }
    if inappropriate_failed {
        builder.fail(
            ValidationCheckName::ObviousInappropriateContentHeuristic,
            "obvious_inappropriate_content_heuristic_failed",
        );
    }
    if secret_failed {
        builder.fail(
            ValidationCheckName::SecretLikeValueHeuristic,
            "secret_like_value_heuristic_failed",
        );
    }
}

fn validate_checker(
    bundle_root: &Path,
    bundle: &Option<InspectedBundle>,
    execution_mode: CheckerExecutionMode,
    builder: &mut ReportBuilder,
) {
    let has_checker = bundle
        .as_ref()
        .is_some_and(|bundle| bundle.root_files.contains("checker.py"));
    if has_checker {
        let Some(checker) = read_utf8_file(bundle_root, "checker.py") else {
            builder.fail(
                ValidationCheckName::PythonCheckerStaticSafety,
                "python_checker_static_safety_failed",
            );
            builder.fail(
                ValidationCheckName::NoExternalNetworkStatic,
                "no_external_network_static_failed",
            );
            return;
        };
        let safety = checker_static_safety(&checker);
        if !safety.safe {
            builder.fail(
                ValidationCheckName::PythonCheckerStaticSafety,
                "python_checker_static_safety_failed",
            );
        }
        if !safety.no_external_network {
            builder.fail(
                ValidationCheckName::NoExternalNetworkStatic,
                "no_external_network_static_failed",
            );
        }
    }

    match execution_mode {
        CheckerExecutionMode::StaticOnly => {
            builder.skip_static_only(
                ValidationCheckName::PythonCheckerRuns,
                "python_checker_runs_skipped_static_only",
            );
        }
    }
}

fn manifest_schema_values_are_valid(manifest: &ArtifactManifest) -> bool {
    valid_id_claim(&manifest.artifact_id, "art_")
        && valid_id_claim(&manifest.request_id, "req_")
        && manifest.work_packet_ids.len() == 1
        && manifest
            .work_packet_ids
            .iter()
            .all(|work_packet_id| valid_id_claim(work_packet_id, "wp_"))
        && safe_length(&manifest.title, 1, 120)
        && !unsafe_public_text(&manifest.title)
        && manifest.subject == "physics"
        && manifest.topic == "conservation_of_energy"
        && manifest.age_range == "14-16"
        && manifest.language == "en"
        && manifest.license == "CC-BY-4.0"
        && manifest.status_claim == "draft_generated"
        && manifest.ai_assisted
        && manifest.contents
            == ALLOWED_FILES
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        && (1..=8).contains(&manifest.known_limitations.len())
        && manifest
            .known_limitations
            .iter()
            .all(|value| safe_length(value, 1, 240) && !unsafe_public_text(value))
}

#[derive(Debug, Clone, Copy)]
struct CheckerStaticSafety {
    safe: bool,
    no_external_network: bool,
}

fn checker_static_safety(source: &str) -> CheckerStaticSafety {
    let normalized = source.to_ascii_lowercase();
    let blocked_tokens = [
        "__",
        "builtins",
        "open",
        "exec",
        "eval",
        "compile",
        "__import__",
        "input",
        "globals",
        "locals",
        "vars",
        "dir",
        "getattr",
        "setattr",
        "delattr",
        "print",
        "while true",
        "for ",
    ];
    let blocked_modules = [
        "subprocess",
        "socket",
        "ssl",
        "http",
        "urllib",
        "requests",
        "pathlib",
        "os",
        "sys",
        "shutil",
        "tempfile",
        "multiprocessing",
        "threading",
        "ctypes",
        "importlib",
    ];
    let dangerous_token = normalized.contains("__")
        || blocked_tokens.iter().any(|token| {
            contains_word_token(&normalized, token) || normalized.contains(&format!("{token}("))
        });
    let dangerous_module = blocked_modules.iter().any(|module| {
        normalized.contains(&format!("import {module}"))
            || normalized.contains(&format!("from {module}"))
    });
    let unsafe_shebang = source.lines().any(unsafe_shebang);
    let top_level_unsafe = source.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            return false;
        }
        trimmed != "import math" && !trimmed.starts_with("def ")
    });
    let required_functions = required_checker_functions_are_defined(source);
    let no_external_network = ![
        "socket", "ssl", "http", "urllib", "requests", "connect(", "urlopen",
    ]
    .iter()
    .any(|token| normalized.contains(token));

    CheckerStaticSafety {
        safe: required_functions
            && !dangerous_token
            && !dangerous_module
            && !top_level_unsafe
            && !unsafe_shebang,
        no_external_network,
    }
}

fn read_utf8_file(bundle_root: &Path, allowed_name: &str) -> Option<String> {
    let bytes = fs::read(bundle_root.join(allowed_name)).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn path_name_is_safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && !name.starts_with('.')
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
        && !name.chars().any(|character| {
            character.is_control()
                || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
}

fn max_size_for_file(name: &str) -> Option<u64> {
    match name {
        "manifest.json" => Some(16 * 1024),
        "worksheet.md" | "answer_key.md" | "teacher_notes.md" => Some(64 * 1024),
        "checker.py" => Some(32 * 1024),
        _ => None,
    }
}

fn safe_length(value: &str, min: usize, max: usize) -> bool {
    let len = value.chars().count();
    (min..=max).contains(&len)
}

fn valid_id_claim(value: &str, prefix: &str) -> bool {
    let Some(suffix) = value.strip_prefix(prefix) else {
        return false;
    };
    !suffix.is_empty()
        && suffix.len() <= 96
        && suffix.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
        })
}

fn unsafe_shebang(line: &str) -> bool {
    let Some(command) = line.trim_start().strip_prefix("#!") else {
        return false;
    };
    let command = command.trim();
    command.is_empty()
        || command.contains('/')
        || command.contains('\\')
        || command.contains(':')
        || unsafe_public_text(command)
}

fn required_checker_functions_are_defined(source: &str) -> bool {
    let mut found = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("def ") {
            continue;
        }
        if trimmed.starts_with("def kinetic_energy(") {
            found.insert("kinetic_energy");
        } else if trimmed.starts_with("def gravitational_potential_energy(") {
            found.insert("gravitational_potential_energy");
        } else if trimmed.starts_with("def speed_from_kinetic_energy(") {
            found.insert("speed_from_kinetic_energy");
        }
    }
    found.len() == 3
}

fn contains_word_token(value: &str, token: &str) -> bool {
    let mut offset = 0;
    while offset < value.len() {
        let Some((relative_start, _)) = value[offset..]
            .char_indices()
            .find(|(_, character)| character.is_ascii_alphabetic() || *character == '_')
        else {
            return false;
        };
        let start = offset + relative_start;
        let mut end = start;
        for (relative_index, character) in value[start..].char_indices() {
            if character.is_ascii_alphanumeric() || character == '_' {
                end = start + relative_index + character.len_utf8();
            } else {
                break;
            }
        }
        if &value[start..end] == token {
            return true;
        }
        offset = end;
    }
    false
}

fn unsafe_markdown(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains('<')
        || normalized.contains("http://")
        || normalized.contains("https://")
        || normalized.contains("data:")
        || normalized.contains("](")
        || normalized.contains("/users/")
        || normalized.contains("/home/")
        || normalized.contains("/etc/")
        || normalized.contains("/private/")
        || normalized.contains("/var/")
        || normalized.contains("/tmp/")
        || normalized.contains("c:\\")
        || normalized.contains("provider ")
        || normalized.contains("model ")
        || normalized.contains("prompt")
        || normalized.contains("raw transcript")
        || normalized.contains("transcript:")
        || normalized.contains("quota")
        || normalized.contains("../")
        || normalized.contains("~/")
        || secret_like(value)
        || pii_like(value)
        || inappropriate_content(value)
}

fn unsafe_public_text(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains("http://")
        || normalized.contains("https://")
        || normalized.contains('@')
        || normalized.contains("/users/")
        || normalized.contains("/home/")
        || normalized.contains("/etc/")
        || normalized.contains("/private/")
        || normalized.contains("/var/")
        || normalized.contains("/tmp/")
        || normalized.contains("../")
        || normalized.contains("~/")
        || normalized.contains("c:\\")
        || normalized.contains("provider ")
        || normalized.contains("model ")
        || normalized.contains("prompt")
        || normalized.contains("raw transcript")
        || normalized.contains("transcript:")
        || normalized.contains("quota")
        || value.chars().any(char::is_control)
        || secret_like(value)
        || pii_like(value)
        || inappropriate_content(value)
}

fn secret_like(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "secret",
        "token",
        "cookie",
        "credential",
        "password",
        "sk-",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn pii_like(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "student ",
        "student:",
        "student record",
        "named student",
        "grade",
        "scored",
        "placement",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn inappropriate_content(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "sexual",
        "graphic violence",
        "self-harm",
        "suicide",
        "slur",
        "weapon construction",
        "make a weapon",
        "privacy invasion",
        "illicit",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn safe_location_for_check(check: ValidationCheckName) -> &'static str {
    match check {
        ValidationCheckName::ManifestSchema
        | ValidationCheckName::LicenseMetadata
        | ValidationCheckName::AiAssistanceDisclosure
        | ValidationCheckName::ManifestLineage
        | ValidationCheckName::ContentsMatchFiles
        | ValidationCheckName::PublicProvenanceAllowlist => "manifest.json",
        ValidationCheckName::MarkdownSafety
        | ValidationCheckName::ObviousPiiHeuristic
        | ValidationCheckName::ObviousInappropriateContentHeuristic
        | ValidationCheckName::SecretLikeValueHeuristic => "public_text",
        ValidationCheckName::PythonCheckerStaticSafety
        | ValidationCheckName::PythonCheckerRuns
        | ValidationCheckName::NoExternalNetworkStatic => "checker.py",
        ValidationCheckName::BundleShape
        | ValidationCheckName::PathNormalization
        | ValidationCheckName::RequiredFiles
        | ValidationCheckName::FileSizeLimits
        | ValidationCheckName::Utf8Text => "bundle_root",
    }
}

fn safe_location_is_allowlisted(location: &str) -> bool {
    matches!(
        location,
        "manifest.json"
            | "worksheet.md"
            | "answer_key.md"
            | "checker.py"
            | "teacher_notes.md"
            | "bundle_root"
            | "public_text"
    )
}
