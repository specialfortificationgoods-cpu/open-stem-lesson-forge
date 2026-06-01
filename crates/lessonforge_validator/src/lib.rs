use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(not(unix))]
compile_error!("lessonforge_validator currently requires Unix filesystem metadata semantics");

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
const ALLOWED_SUBJECTS: [&str; 1] = ["physics"];
const ALLOWED_TOPICS: [&str; 1] = ["conservation_of_energy"];
const ALLOWED_AGE_RANGES: [&str; 1] = ["14-16"];
const ALLOWED_LANGUAGES: [&str; 1] = ["en"];
const DIGEST_FILE_ORDER: [&str; 5] = [
    "answer_key.md",
    "checker.py",
    "manifest.json",
    "teacher_notes.md",
    "worksheet.md",
];
const MARKDOWN_FILES: [&str; 3] = ["worksheet.md", "answer_key.md", "teacher_notes.md"];
const WHOLE_BUNDLE_MAX_BYTES: u64 = 256 * 1024;
const MAX_LITERAL_FOR_LOOP_ITEMS: usize = 50;

pub fn crate_boundary() -> &'static str {
    VALIDATOR_NAME
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckerExecutionMode {
    StaticOnly,
    SandboxedSubprocess,
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
    ArtifactDigestIntegrity,
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
            Self::ArtifactDigestIntegrity => "artifact_digest_integrity",
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

const CHECK_ORDER: [ValidationCheckName; 19] = [
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
    ValidationCheckName::ArtifactDigestIntegrity,
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
                ValidationReportStatus::Failed => "trusted_failed",
                ValidationReportStatus::IncompleteStaticOnly => "trusted_incomplete_static_only",
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
                ValidationCheckName::ArtifactDigestIntegrity,
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
            ValidationCheckName::ArtifactDigestIntegrity,
            "artifact_digest_mismatch",
        );
    }
    let manifest = validate_manifest(bundle_root, &bundle, context, &mut builder);
    validate_markdown(bundle_root, &bundle, &manifest, &mut builder);
    let execution_prerequisites_passed = builder.all_passed(&[
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
        ValidationCheckName::ArtifactDigestIntegrity,
        ValidationCheckName::MarkdownSafety,
        ValidationCheckName::ObviousPiiHeuristic,
        ValidationCheckName::ObviousInappropriateContentHeuristic,
        ValidationCheckName::SecretLikeValueHeuristic,
    ]);
    validate_checker(
        bundle_root,
        &bundle,
        context.execution_mode,
        execution_prerequisites_passed,
        &mut builder,
    );
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
        let total_size = file_digests
            .iter()
            .fold(0u64, |total, record: &FileDigestRecord| {
                total.saturating_add(record.size_bytes)
            })
            .saturating_add(metadata.len());
        if !file_size_within_limits(path, metadata.len(), total_size) {
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
    let bytes = serde_json_canonicalizer::to_vec(&canonical)
        .map_err(|_| ValidatorError::DigestUnavailable)?;
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

    fn all_passed(&self, checks: &[ValidationCheckName]) -> bool {
        checks.iter().all(|check| {
            self.report
                .checks
                .iter()
                .any(|record| record.check == *check && record.status == CheckStatus::Passed)
        })
    }

    fn finish(mut self, authoritative_digests: ArtifactDigestSet) -> ArtifactValidationReport {
        self.report.authoritative_digests = authoritative_digests;
        self.refresh_failures_and_status();
        if public_provenance_is_allowlisted(&self.report.public_provenance()) {
            self.set(
                ValidationCheckName::PublicProvenanceAllowlist,
                CheckStatus::Passed,
                "public_provenance_allowlist_passed",
            );
        } else {
            self.fail(
                ValidationCheckName::PublicProvenanceAllowlist,
                "public_provenance_allowlist_failed",
            );
        }
        self.refresh_failures_and_status();
        self.report
    }

    fn refresh_failures_and_status(&mut self) {
        self.report.failures = self
            .report
            .checks
            .iter()
            .filter(|record| record.status == CheckStatus::Failed)
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
    }
}

fn public_provenance_is_allowlisted(provenance: &PublicProvenance) -> bool {
    provenance.generated_by_category == "runner_assisted"
        && provenance.validation_category == VALIDATOR_NAME
        && matches!(
            provenance.validation_state_summary.as_str(),
            "trusted_passed" | "trusted_failed" | "trusted_incomplete_static_only"
        )
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
        let oversized = !file_size_within_limits(&name, metadata.len(), bundle.total_size);
        if oversized {
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
        if oversized {
            continue;
        }
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
        .is_none_or(|bundle| !bundle.text_files.contains("manifest.json"))
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
    let Some(bundle) = bundle else {
        return;
    };
    let mut pii_failed = false;
    let mut inappropriate_failed = false;
    let mut secret_failed = false;
    for file in MARKDOWN_FILES {
        if !bundle.root_files.contains(file) || !bundle.text_files.contains(file) {
            continue;
        }
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
    execution_prerequisites_passed: bool,
    builder: &mut ReportBuilder,
) {
    let mut checker_static_ok = false;
    let mut checker_no_network_ok = false;
    let has_checker = bundle
        .as_ref()
        .is_some_and(|bundle| bundle.text_files.contains("checker.py"));
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
            match execution_mode {
                CheckerExecutionMode::StaticOnly => {
                    builder.skip_static_only(
                        ValidationCheckName::PythonCheckerRuns,
                        "python_checker_runs_skipped_static_only",
                    );
                }
                CheckerExecutionMode::SandboxedSubprocess => {
                    builder.fail(
                        ValidationCheckName::PythonCheckerRuns,
                        "python_checker_runs_failed",
                    );
                }
            }
            return;
        };
        let safety = checker_static_safety(&checker);
        checker_static_ok = safety.safe;
        checker_no_network_ok = safety.no_external_network;
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
        CheckerExecutionMode::SandboxedSubprocess => {
            if !has_checker
                || !execution_prerequisites_passed
                || !checker_static_ok
                || !checker_no_network_ok
                || !run_python_checker(bundle_root)
            {
                builder.fail(
                    ValidationCheckName::PythonCheckerRuns,
                    "python_checker_runs_failed",
                );
            }
        }
    }
}

fn run_python_checker(bundle_root: &Path) -> bool {
    let Ok(bundle_root) = bundle_root.canonicalize() else {
        return false;
    };
    let harness = r#"
import math
import resource
import signal
import sys

resource.setrlimit(resource.RLIMIT_CPU, (1, 1))
signal.alarm(2)
resource.setrlimit(resource.RLIMIT_NOFILE, (16, 16))
sys.path.insert(0, ".")
import checker
resource.setrlimit(resource.RLIMIT_NOFILE, (3, 3))

def close(actual, expected):
    return math.isfinite(actual) and abs(actual - expected) <= 1e-9

checks = [
    close(checker.kinetic_energy(2.0, 3.0), 9.0),
    (
        close(checker.gravitational_potential_energy(2.0, 9.8, 5.0), 98.0)
        or close(checker.gravitational_potential_energy(2.0, 5.0, 9.8), 98.0)
    ),
    (
        close(checker.speed_from_kinetic_energy(9.0, 2.0), 3.0)
        or close(checker.speed_from_kinetic_energy(2.0, 9.0), 3.0)
    ),
]
if not all(checks):
    raise SystemExit(1)
"#;
    let Some(python3) = find_python3_interpreter() else {
        return false;
    };
    let Ok(output) = Command::new(python3)
        .arg("-I")
        .arg("-B")
        .arg("-c")
        .arg(harness)
        .current_dir(bundle_root)
        .env_clear()
        .output()
    else {
        return false;
    };
    output.status.success() && output.stdout.is_empty() && output.stderr.is_empty()
}

fn find_python3_interpreter() -> Option<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join("python3"))
        .chain(
            [
                "/usr/bin/python3",
                "/opt/homebrew/bin/python3",
                "/usr/local/bin/python3",
            ]
            .into_iter()
            .map(PathBuf::from),
        )
        .filter_map(|candidate| candidate.canonicalize().ok())
        .find(|candidate| candidate.is_absolute() && candidate.is_file())
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
        && ALLOWED_SUBJECTS.contains(&manifest.subject.as_str())
        && ALLOWED_TOPICS.contains(&manifest.topic.as_str())
        && ALLOWED_AGE_RANGES.contains(&manifest.age_range.as_str())
        && ALLOWED_LANGUAGES.contains(&manifest.language.as_str())
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
    python_ast_checker_static_safety(source)
        .unwrap_or_else(|| heuristic_checker_static_safety(source))
}

fn python_ast_checker_static_safety(source: &str) -> Option<CheckerStaticSafety> {
    let python3 = find_python3_interpreter()?;
    let script = r#"
import ast
import sys

source = sys.stdin.read()
blocked_modules = {
    "subprocess", "socket", "ssl", "http", "urllib", "requests", "pathlib",
    "pickle", "shelve", "os", "sys", "shutil", "tempfile", "multiprocessing",
    "threading", "ctypes", "pty", "fcntl", "signal", "gc", "inspect", "dis",
    "importlib",
}
network_modules = {"socket", "ssl", "http", "urllib", "requests"}
blocked_calls = {
    "builtins", "open", "file", "__file__", "__name__", "exec", "eval", "compile",
    "__import__", "input", "breakpoint", "pdb", "help", "resource", "globals",
    "locals", "vars", "dir", "getattr", "setattr", "delattr", "iter", "range",
    "print",
}
required = {"kinetic_energy", "gravitational_potential_energy", "speed_from_kinetic_energy"}
safe = True
no_external_network = True
try:
    tree = ast.parse(source)
except SyntaxError:
    print("0 0")
    raise SystemExit(0)

found = set()
for index, node in enumerate(tree.body):
    if isinstance(node, ast.Import):
        names = {alias.name.split(".")[0] for alias in node.names}
        if names != {"math"} or any(alias.asname for alias in node.names):
            safe = False
        if names & blocked_modules:
            safe = False
        if names & network_modules:
            no_external_network = False
    elif isinstance(node, ast.ImportFrom):
        module = (node.module or "").split(".")[0]
        safe = False
        if module in network_modules:
            no_external_network = False
    elif isinstance(node, ast.FunctionDef):
        if node.name not in required or node.decorator_list:
            safe = False
        found.add(node.name)
    elif index == 0 and isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
        pass
    else:
        safe = False

class Visitor(ast.NodeVisitor):
    def visit_Name(self, node):
        global safe
        if "__" in node.id or node.id in blocked_calls or node.id in blocked_modules:
            safe = False
        self.generic_visit(node)

    def visit_Attribute(self, node):
        global safe
        if "__" in node.attr:
            safe = False
        self.generic_visit(node)

    def visit_Import(self, node):
        global safe, no_external_network
        names = {alias.name.split(".")[0] for alias in node.names}
        if names & blocked_modules or any(alias.asname for alias in node.names):
            safe = False
        if names & network_modules:
            no_external_network = False
        self.generic_visit(node)

    def visit_ImportFrom(self, node):
        global safe, no_external_network
        module = (node.module or "").split(".")[0]
        if module in blocked_modules:
            safe = False
        if module in network_modules:
            no_external_network = False
        self.generic_visit(node)

    def visit_Call(self, node):
        global safe, no_external_network
        name = ""
        if isinstance(node.func, ast.Name):
            name = node.func.id
        elif isinstance(node.func, ast.Attribute):
            name = node.func.attr
            base = node.func.value
            if isinstance(base, ast.Name) and base.id in network_modules:
                no_external_network = False
        if name in blocked_calls:
            safe = False
        if name in {"connect", "urlopen"}:
            safe = False
            no_external_network = False
        self.generic_visit(node)

    def visit_JoinedStr(self, node):
        global safe
        safe = False
        self.generic_visit(node)

    def visit_FormattedValue(self, node):
        global safe
        safe = False
        self.generic_visit(node)

    def visit_While(self, node):
        global safe
        safe = False
        self.generic_visit(node)

    def visit_For(self, node):
        global safe
        if not isinstance(node.iter, (ast.List, ast.Tuple)) or len(node.iter.elts) > 50:
            safe = False
        self.generic_visit(node)

Visitor().visit(tree)
if not required.issubset(found):
    safe = False
print(("1" if safe else "0") + " " + ("1" if no_external_network else "0"))
"#;
    let mut child = Command::new(python3)
        .arg("-I")
        .arg("-B")
        .arg("-c")
        .arg(script)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.as_mut()?.write_all(source.as_bytes()).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let rendered = String::from_utf8(output.stdout).ok()?;
    let mut parts = rendered.split_whitespace();
    Some(CheckerStaticSafety {
        safe: parts.next()? == "1" && !source.lines().any(unsafe_shebang),
        no_external_network: parts.next()? == "1",
    })
}

fn heuristic_checker_static_safety(source: &str) -> CheckerStaticSafety {
    let code_without_literals = strip_python_comments_and_strings(source);
    let joined_code = collapse_python_line_continuations(&code_without_literals);
    let normalized = joined_code.to_ascii_lowercase();
    let blocked_tokens = [
        "builtins",
        "open",
        "file",
        "__file__",
        "__name__",
        "exec",
        "eval",
        "compile",
        "__import__",
        "input",
        "breakpoint",
        "pdb",
        "help",
        "resource",
        "globals",
        "locals",
        "vars",
        "dir",
        "getattr",
        "setattr",
        "delattr",
        "iter",
        "range",
        "print",
    ];
    let blocked_modules = [
        "subprocess",
        "socket",
        "ssl",
        "http",
        "urllib",
        "requests",
        "pathlib",
        "pickle",
        "shelve",
        "os",
        "sys",
        "shutil",
        "tempfile",
        "multiprocessing",
        "threading",
        "ctypes",
        "pty",
        "fcntl",
        "signal",
        "gc",
        "inspect",
        "dis",
        "importlib",
    ];
    let dangerous_token = normalized.contains("__")
        || blocked_tokens.iter().any(|token| {
            contains_word_token(&normalized, token) || normalized.contains(&format!("{token}("))
        });
    let dangerous_module = contains_blocked_python_import(&normalized, &blocked_modules);
    let unsafe_shebang = source.lines().any(unsafe_shebang);
    let unsafe_f_string = contains_python_f_string_literal(source);
    let unsafe_loop = contains_unsafe_loop_construct(&normalized);
    let top_level_unsafe = joined_code.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            return false;
        }
        trimmed != "import math" && !trimmed.starts_with("def ")
    });
    let required_functions = required_checker_functions_are_defined(&code_without_literals);
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
            && !unsafe_f_string
            && !unsafe_loop
            && !unsafe_shebang,
        no_external_network,
    }
}

fn collapse_python_line_continuations(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut continuation = false;
    for line in source.lines() {
        let trimmed_end = line.trim_end();
        let line_continues = trimmed_end.ends_with('\\');
        let segment = if line_continues {
            &trimmed_end[..trimmed_end.len().saturating_sub(1)]
        } else {
            line
        };
        if continuation {
            output.push(' ');
            output.push_str(segment.trim_start());
        } else {
            output.push_str(segment);
        }
        if line_continues {
            continuation = true;
        } else {
            output.push('\n');
            continuation = false;
        }
    }
    if continuation {
        output.push('\n');
    }
    output
}

fn contains_unsafe_loop_construct(normalized_code: &str) -> bool {
    normalized_code.lines().any(|line| {
        let trimmed = line.trim_start();
        if contains_python_keyword(trimmed, "while") {
            return true;
        }
        if let Some(after_for) = python_keyword_tail(trimmed, "for")
            && finite_literal_for_loop_tail(after_for)
        {
            return false;
        }
        contains_python_keyword(trimmed, "for")
    })
}

fn contains_blocked_python_import(normalized_code: &str, blocked_modules: &[&str]) -> bool {
    normalized_code.lines().any(|line| {
        line.split([';', ':']).any(|statement| {
            let trimmed = statement.trim_start();
            if let Some(after_import) = python_keyword_tail_after_space(trimmed, "import") {
                return after_import
                    .split(',')
                    .filter_map(take_import_module_name)
                    .any(|module| blocked_modules.contains(&module));
            }
            if let Some(after_from) = python_keyword_tail_after_space(trimmed, "from")
                && let Some(module) = take_import_module_name(after_from)
            {
                return blocked_modules.contains(&module);
            }
            false
        })
    })
}

fn take_import_module_name(value: &str) -> Option<&str> {
    let value = value.trim_start();
    let mut end = 0;
    for (index, character) in value.char_indices() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '.' {
            end = index + character.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let module_path = &value[..end];
    let module = module_path.split('.').next().unwrap_or(module_path);
    if module.is_empty() {
        None
    } else {
        Some(module)
    }
}

fn python_keyword_tail_after_space<'a>(value: &'a str, keyword: &str) -> Option<&'a str> {
    let tail = value.strip_prefix(keyword)?;
    if tail
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_whitespace())
    {
        Some(tail)
    } else {
        None
    }
}

fn finite_literal_for_loop_tail(after_for: &str) -> bool {
    let after_for = after_for.trim_start();
    let Some((target, after_target)) = take_python_identifier(after_for) else {
        return false;
    };
    let after_target = after_target.trim_start();
    let Some(after_in) = python_keyword_tail(after_target, "in") else {
        return false;
    };
    if !valid_python_identifier(target) {
        return false;
    }
    let Some(iterable) = after_in.trim_start().strip_suffix(':') else {
        return false;
    };
    let iterable = iterable.trim();
    if !(iterable.starts_with('[') && iterable.ends_with(']')) {
        return false;
    }
    let items = &iterable[1..iterable.len().saturating_sub(1)];
    if items.len() > 256 {
        return false;
    }
    let mut item_count = 0usize;
    for item in items
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        item_count += 1;
        if item_count > MAX_LITERAL_FOR_LOOP_ITEMS
            || !(valid_python_identifier(item) || valid_numeric_literal(item))
        {
            return false;
        }
    }
    true
}

fn python_keyword_tail<'a>(value: &'a str, keyword: &str) -> Option<&'a str> {
    let tail = value.strip_prefix(keyword)?;
    if tail
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    Some(tail)
}

fn contains_python_keyword(value: &str, keyword: &str) -> bool {
    let mut offset = 0;
    while offset < value.len() {
        let Some(relative_start) = value[offset..].find(keyword) else {
            return false;
        };
        let start = offset + relative_start;
        let end = start + keyword.len();
        let before_is_identifier = value[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
        let after_is_identifier = value[end..]
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
        if !before_is_identifier && !after_is_identifier {
            return true;
        }
        offset = end;
    }
    false
}

fn take_python_identifier(value: &str) -> Option<(&str, &str)> {
    let mut chars = value.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut end = first.len_utf8();
    for (index, character) in chars {
        if character.is_ascii_alphanumeric() || character == '_' {
            end = index + character.len_utf8();
        } else {
            break;
        }
    }
    Some((&value[..end], &value[end..]))
}

fn valid_python_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn valid_numeric_literal(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    if value.is_empty() {
        return false;
    }
    let mut decimal_seen = false;
    let mut digit_seen = false;
    for character in value.chars() {
        if character.is_ascii_digit() {
            digit_seen = true;
        } else if character == '.' && !decimal_seen {
            decimal_seen = true;
        } else {
            return false;
        }
    }
    digit_seen
}

fn contains_python_f_string_literal(source: &str) -> bool {
    let mut state = PythonStringState::Normal;
    for line in source.lines() {
        let chars = line.chars().collect::<Vec<_>>();
        let mut index = 0;
        while index < chars.len() {
            match state {
                PythonStringState::Normal => {
                    let character = chars[index];
                    if character == '#' {
                        break;
                    }
                    if character == '\'' || character == '"' {
                        if string_prefix_before_quote(&chars, index)
                            .is_some_and(|prefix| matches!(prefix, "f" | "fr" | "rf"))
                        {
                            return true;
                        }
                        if index + 2 < chars.len()
                            && chars[index + 1] == character
                            && chars[index + 2] == character
                        {
                            state = PythonStringState::Triple(character);
                            index += 3;
                        } else {
                            state = PythonStringState::Single(character);
                            index += 1;
                        }
                    } else {
                        index += 1;
                    }
                }
                PythonStringState::Single(quote) => {
                    if chars[index] == '\\' {
                        index += 1;
                        if index < chars.len() {
                            index += 1;
                        }
                    } else {
                        if chars[index] == quote {
                            state = PythonStringState::Normal;
                        }
                        index += 1;
                    }
                }
                PythonStringState::Triple(quote) => {
                    if index + 2 < chars.len()
                        && chars[index] == quote
                        && chars[index + 1] == quote
                        && chars[index + 2] == quote
                    {
                        state = PythonStringState::Normal;
                        index += 3;
                    } else {
                        index += 1;
                    }
                }
            }
        }
        if matches!(state, PythonStringState::Single(_)) {
            state = PythonStringState::Normal;
        }
    }
    false
}

fn string_prefix_before_quote(chars: &[char], quote_index: usize) -> Option<&'static str> {
    let mut start = quote_index;
    while start > 0 && chars[start - 1].is_ascii_alphabetic() {
        start -= 1;
    }
    match chars[start..quote_index]
        .iter()
        .collect::<String>()
        .to_ascii_lowercase()
        .as_str()
    {
        "f" => Some("f"),
        "fr" => Some("fr"),
        "rf" => Some("rf"),
        _ => None,
    }
}

fn strip_python_comments_and_strings(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut state = PythonStringState::Normal;
    for line in source.lines() {
        let chars = line.chars().collect::<Vec<_>>();
        let mut index = 0;
        while index < chars.len() {
            match state {
                PythonStringState::Normal => {
                    let character = chars[index];
                    if character == '#' {
                        break;
                    }
                    if let Some(prefix_len) = python_string_prefix_len_at(&chars, index) {
                        for _ in 0..prefix_len {
                            output.push(' ');
                        }
                        index += prefix_len;
                    } else if character == '\'' || character == '"' {
                        if index + 2 < chars.len()
                            && chars[index + 1] == character
                            && chars[index + 2] == character
                        {
                            state = PythonStringState::Triple(character);
                            output.push(' ');
                            output.push(' ');
                            output.push(' ');
                            index += 3;
                        } else {
                            state = PythonStringState::Single(character);
                            output.push(' ');
                            index += 1;
                        }
                    } else {
                        output.push(character);
                        index += 1;
                    }
                }
                PythonStringState::Single(quote) => {
                    if chars[index] == '\\' {
                        output.push(' ');
                        index += 1;
                        if index < chars.len() {
                            output.push(' ');
                            index += 1;
                        }
                    } else {
                        if chars[index] == quote {
                            state = PythonStringState::Normal;
                        }
                        output.push(' ');
                        index += 1;
                    }
                }
                PythonStringState::Triple(quote) => {
                    if index + 2 < chars.len()
                        && chars[index] == quote
                        && chars[index + 1] == quote
                        && chars[index + 2] == quote
                    {
                        state = PythonStringState::Normal;
                        output.push(' ');
                        output.push(' ');
                        output.push(' ');
                        index += 3;
                    } else {
                        output.push(' ');
                        index += 1;
                    }
                }
            }
        }
        let line_continues_string =
            matches!(state, PythonStringState::Single(_)) && line.ends_with('\\');
        if matches!(state, PythonStringState::Single(_)) && !line_continues_string {
            state = PythonStringState::Normal;
        }
        output.push('\n');
    }
    output
}

fn python_string_prefix_len_at(chars: &[char], start: usize) -> Option<usize> {
    if !chars
        .get(start)
        .is_some_and(|character| character.is_ascii_alphabetic())
    {
        return None;
    }
    let mut end = start;
    while chars
        .get(end)
        .is_some_and(|character| character.is_ascii_alphabetic())
    {
        end += 1;
    }
    if !chars
        .get(end)
        .is_some_and(|character| *character == '\'' || *character == '"')
    {
        return None;
    }
    let prefix = chars[start..end]
        .iter()
        .collect::<String>()
        .to_ascii_lowercase();
    if matches!(
        prefix.as_str(),
        "r" | "u" | "b" | "f" | "br" | "rb" | "fr" | "rf"
    ) {
        Some(end - start)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PythonStringState {
    Normal,
    Single(char),
    Triple(char),
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

fn file_size_within_limits(name: &str, file_size: u64, total_size: u64) -> bool {
    file_size <= max_size_for_file(name).unwrap_or(0) && total_size <= WHOLE_BUNDLE_MAX_BYTES
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
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
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
        || provider_trace_like(value, &normalized)
        || prompt_trace_like(&normalized)
        || contains_word_or_phrase(&normalized, "raw transcript")
        || normalized.contains("transcript:")
        || quota_trace_like(&normalized)
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
        || provider_trace_like(value, &normalized)
        || prompt_trace_like(&normalized)
        || contains_word_or_phrase(&normalized, "raw transcript")
        || normalized.contains("transcript:")
        || quota_trace_like(&normalized)
        || value.chars().any(char::is_control)
        || secret_like(value)
        || pii_like(value)
        || inappropriate_content(value)
}

fn secret_like(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains("sk-")
        || assignment_like(&normalized, "api_key")
        || assignment_like(&normalized, "apikey")
        || assignment_like(&normalized, "token")
        || assignment_like(&normalized, "password")
        || assignment_like(&normalized, "credential")
        || assignment_like(&normalized, "cookie")
        || assignment_like(&normalized, "secret")
        || contains_word_or_phrase(&normalized, "authorization bearer")
        || normalized.trim_start().starts_with("bearer ")
        || normalized.contains(" bearer ")
}

fn pii_like(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    contains_word_or_phrase(&normalized, "student id")
        || contains_word_or_phrase(&normalized, "student record")
        || contains_word_or_phrase(&normalized, "named student")
        || student_score_context_like(value, &normalized)
}

fn provider_trace_like(value: &str, normalized: &str) -> bool {
    contains_word_or_phrase(normalized, "generated with provider")
        || (contains_word_or_phrase(normalized, "provider")
            && contains_word_or_phrase(normalized, "model")
            && value.split_whitespace().count() <= 16)
}

fn prompt_trace_like(normalized: &str) -> bool {
    contains_word_or_phrase(normalized, "prompt path")
        || contains_word_or_phrase(normalized, "raw prompt")
        || normalized.trim_start().starts_with("prompt:")
        || contains_word_or_phrase(normalized, "ignore previous instructions")
        || contains_word_or_phrase(normalized, "reveal hidden answers")
        || normalized.contains("/prompts/")
}

fn quota_trace_like(normalized: &str) -> bool {
    contains_word_or_phrase(normalized, "quota remaining")
        || contains_word_or_phrase(normalized, "exact quota")
}

fn assignment_like(normalized: &str, key: &str) -> bool {
    normalized.match_indices(key).any(|(index, _)| {
        let tail = normalized[index + key.len()..].trim_start();
        tail.starts_with('=') || tail.starts_with(':')
    })
}

fn student_score_context_like(value: &str, normalized: &str) -> bool {
    (contains_word_or_phrase(normalized, "student")
        || contains_word_or_phrase(normalized, "learner")
        || contains_word_or_phrase(normalized, "roster")
        || contains_word_or_phrase(normalized, "attendance")
        || contains_word_or_phrase(normalized, "grade record")
        || contains_word_or_phrase(normalized, "class list"))
        && contains_titlecase_name_pair(value)
}

fn contains_titlecase_name_pair(value: &str) -> bool {
    let mut previous_was_name = false;
    for word in value.split(|character: char| !character.is_ascii_alphabetic()) {
        if word.is_empty() {
            continue;
        }
        let current_is_name = titlecase_word_shape(word);
        if previous_was_name && current_is_name {
            return true;
        }
        previous_was_name = current_is_name;
    }
    false
}

fn titlecase_word_shape(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    word.len() >= 2 && first.is_ascii_uppercase() && chars.all(|ch| ch.is_ascii_lowercase())
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
    .any(|needle| contains_word_or_phrase(&normalized, needle))
}

fn contains_word_or_phrase(normalized: &str, needle: &str) -> bool {
    normalized.match_indices(needle).any(|(index, _)| {
        let before = normalized[..index].chars().next_back();
        let after = normalized[index + needle.len()..].chars().next();
        before.is_none_or(|character| !character.is_ascii_alphanumeric())
            && after.is_none_or(|character| !character.is_ascii_alphanumeric())
    })
}

fn safe_location_for_check(check: ValidationCheckName) -> &'static str {
    match check {
        ValidationCheckName::ManifestSchema
        | ValidationCheckName::LicenseMetadata
        | ValidationCheckName::AiAssistanceDisclosure
        | ValidationCheckName::ManifestLineage
        | ValidationCheckName::ArtifactDigestIntegrity
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

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CHECKER_SOURCE: &str = r#"
import math

def kinetic_energy(mass_kg: float, speed_m_per_s: float) -> float:
    return 0.5 * mass_kg * speed_m_per_s ** 2

def gravitational_potential_energy(mass_kg: float, g_m_per_s2: float, height_m: float) -> float:
    return mass_kg * g_m_per_s2 * height_m

def speed_from_kinetic_energy(kinetic_energy_j: float, mass_kg: float) -> float:
    return math.sqrt((2.0 * kinetic_energy_j) / mass_kg)
"#;

    #[test]
    fn python_ast_static_safety_path_executes_successfully() -> Result<(), String> {
        let result = python_ast_checker_static_safety(VALID_CHECKER_SOURCE)
            .ok_or_else(|| "python AST static checker should execute".to_owned())?;

        assert!(result.safe);
        assert!(result.no_external_network);
        Ok(())
    }

    #[test]
    fn python_ast_static_safety_rejects_blocked_imports_without_fallback() -> Result<(), String> {
        let result = python_ast_checker_static_safety("import socket\n")
            .ok_or_else(|| "python AST checker should execute".to_owned())?;

        assert!(!result.safe);
        assert!(!result.no_external_network);
        Ok(())
    }

    #[test]
    fn python_ast_static_safety_rejects_decorator_import_time_bypass() -> Result<(), String> {
        let source = r#"
import math

def dec(f):
    o = open
    o("/tmp/lessonforge-unsafe", "w")
    return f

@dec
def kinetic_energy(mass_kg: float, speed_m_per_s: float) -> float:
    return 0.5 * mass_kg * speed_m_per_s ** 2

def gravitational_potential_energy(mass_kg: float, g_m_per_s2: float, height_m: float) -> float:
    return mass_kg * g_m_per_s2 * height_m

def speed_from_kinetic_energy(kinetic_energy_j: float, mass_kg: float) -> float:
    return math.sqrt((2.0 * kinetic_energy_j) / mass_kg)
"#;
        let result = python_ast_checker_static_safety(source)
            .ok_or_else(|| "python AST checker should execute".to_owned())?;

        assert!(!result.safe);
        assert!(result.no_external_network);
        Ok(())
    }
}
