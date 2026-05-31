use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::{DependencyKind, FeatureName, Metadata, MetadataCommand, Package, PackageId};

const REQUIRED_CENTRAL_PACKAGES: &[(&str, &str)] = &[
    ("lessonforge_core", "crates/lessonforge_core"),
    ("lessonforge_api", "crates/lessonforge_api"),
    ("lessonforge_schema", "crates/lessonforge_schema"),
    ("lessonforge_validator", "crates/lessonforge_validator"),
];
const CENTRAL_PATHS: &[&str] = &[
    "crates/lessonforge_core",
    "crates/lessonforge_api",
    "crates/lessonforge_schema",
    "crates/lessonforge_validator",
];

const OPTIONAL_CENTRAL_DATA_PATHS: &[&str] = &[
    "schemas",
    "migrations",
    "crates/lessonforge_api/migrations",
    "crates/lessonforge_api/schema",
    "crates/lessonforge_schema/schemas",
];

const SCANNED_EXTENSIONS: &[&str] = &["rs", "toml", "sql", "json", "jsonl", "yaml", "yml"];

const FORBIDDEN_MARKERS: &[ForbiddenMarker] = &[
    ForbiddenMarker::new("openai"),
    ForbiddenMarker::new("anthropic"),
    ForbiddenMarker::new("azure_openai"),
    ForbiddenMarker::new("azure-openai"),
    ForbiddenMarker::new("gemini"),
    ForbiddenMarker::new("google_ai"),
    ForbiddenMarker::new("google-ai"),
    ForbiddenMarker::new("mistral"),
    ForbiddenMarker::new("ollama"),
    ForbiddenMarker::new("llama"),
    ForbiddenMarker::new("lmstudio"),
    ForbiddenMarker::new("lm_studio"),
    ForbiddenMarker::new("openrouter"),
    ForbiddenMarker::new("langchain"),
    ForbiddenMarker::new("llamaindex"),
    ForbiddenMarker::new("llama_index"),
    ForbiddenMarker::new("model_provider"),
    ForbiddenMarker::new("model-provider"),
    ForbiddenMarker::new("model_moderate"),
    ForbiddenMarker::new("moderation_provider"),
    ForbiddenMarker::new("moderation-provider"),
    ForbiddenMarker::new("embedding"),
    ForbiddenMarker::new("embeddings"),
    ForbiddenMarker::new("vector"),
    ForbiddenMarker::new("qdrant"),
    ForbiddenMarker::new("pinecone"),
    ForbiddenMarker::new("weaviate"),
    ForbiddenMarker::new("milvus"),
    ForbiddenMarker::new("chroma"),
    ForbiddenMarker::new("faiss"),
    ForbiddenMarker::new("semantic_cache"),
    ForbiddenMarker::new("semantic-cache"),
    ForbiddenMarker::new("semantic_merge"),
    ForbiddenMarker::new("call_llm"),
    ForbiddenMarker::new("generate_with_model"),
    ForbiddenMarker::new("plan_with_model"),
    ForbiddenMarker::new("prompt_template"),
    ForbiddenMarker::new("prompt-template"),
    ForbiddenMarker::new("provider_base_url"),
    ForbiddenMarker::new("provider-base-url"),
    ForbiddenMarker::new("provider_account"),
    ForbiddenMarker::new("provider-account"),
    ForbiddenMarker::new("raw_provider_response"),
    ForbiddenMarker::new("raw-provider-response"),
    ForbiddenMarker::new("model_trace"),
    ForbiddenMarker::new("model-trace"),
    ForbiddenMarker::new("browser_profile"),
    ForbiddenMarker::new("browser-profile"),
    ForbiddenMarker::new("playwright"),
    ForbiddenMarker::new("webdriver"),
    ForbiddenMarker::new("headless_chrome"),
    ForbiddenMarker::new("thirtyfour"),
    ForbiddenMarker::new("fantoccini"),
    ForbiddenMarker::new("docker"),
    ForbiddenMarker::new("containerd"),
    ForbiddenMarker::new("candle"),
    ForbiddenMarker::new("tokenizers"),
    ForbiddenMarker::new("hf_hub"),
    ForbiddenMarker::new("hf-hub"),
    ForbiddenMarker::new("async_openai"),
    ForbiddenMarker::new("async-openai"),
    ForbiddenMarker::new("genai"),
    ForbiddenMarker::new("OPENAI_API_KEY"),
    ForbiddenMarker::new("ANTHROPIC_API_KEY"),
    ForbiddenMarker::new("GEMINI_API_KEY"),
    ForbiddenMarker::new("AZURE_OPENAI_API_KEY"),
    ForbiddenMarker::new("GOOGLE_API_KEY"),
    ForbiddenMarker::new("GOOGLE_AI_API_KEY"),
    ForbiddenMarker::new("OPENROUTER_API_KEY"),
    ForbiddenMarker::new("MISTRAL_API_KEY"),
    ForbiddenMarker::new("OLLAMA_HOST"),
    ForbiddenMarker::new("MODEL_PROVIDER"),
    ForbiddenMarker::new("PROVIDER_BASE_URL"),
    ForbiddenMarker::new("MODERATION_PROVIDER_API_KEY"),
    ForbiddenMarker::new("BROWSER_PROFILE_PATH"),
    ForbiddenMarker::new("LOCAL_MODEL_PATH"),
    ForbiddenMarker::new("PROMPT_TEMPLATE_PATH"),
    ForbiddenMarker::new("fn_embed"),
    ForbiddenMarker::new("fn_infer"),
];

const CENTRAL_DATA_FORBIDDEN_MARKERS: &[ForbiddenMarker] = &[
    ForbiddenMarker::new("model"),
    ForbiddenMarker::new("provider"),
    ForbiddenMarker::new("prompt"),
    ForbiddenMarker::new("prompts"),
    ForbiddenMarker::new("model_prompt"),
    ForbiddenMarker::new("prompt_text"),
    ForbiddenMarker::new("raw_prompt"),
];

const DIRECT_INFERENCE_CALL_MARKERS: &[ForbiddenMarker] =
    &[ForbiddenMarker::new("embed"), ForbiddenMarker::new("infer")];

const FORBIDDEN_DEPENDENCY_NAMES: &[&str] = &[
    "candle-core",
    "candle-nn",
    "candle-transformers",
    "chromiumoxide",
    "fantoccini",
    "fastembed",
    "headless-chrome",
    "lancedb",
    "llm",
    "ort",
    "qdrant-client",
    "rig-core",
    "rust-bert",
    "tch",
    "thirtyfour",
    "tract-onnx",
    "wasmtime",
    "wasmer",
];

fn main() {
    match run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let root = parse_root(env::args().skip(1).collect())?;
    let findings = scan_workspace(&root)?;
    if findings.is_empty() {
        println!("no inference markers found in deterministic central crates");
        return Ok(());
    }

    for finding in findings {
        eprintln!(
            "{}:{} contains forbidden marker {} in {}",
            finding.path.display(),
            finding.line,
            finding.marker.name,
            finding.context
        );
    }
    Err("no-inference guardrail failed".to_owned())
}

fn parse_root(args: Vec<String>) -> Result<PathBuf, String> {
    match args.as_slice() {
        [] => env::current_dir().map_err(|error| error.to_string()),
        [flag, root] if flag == "--root" => Ok(PathBuf::from(root)),
        _ => Err("usage: verify-no-inference-core [--root PATH]".to_owned()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ForbiddenMarker {
    name: &'static str,
}

impl ForbiddenMarker {
    const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    path: PathBuf,
    line: usize,
    marker: ForbiddenMarker,
    context: &'static str,
}

fn scan_workspace(root: &Path) -> Result<Vec<Finding>, String> {
    let mut findings = Vec::new();
    fail_closed_if_required_paths_missing(root)?;

    let metadata = workspace_metadata(root)?;
    let package_scan_targets = central_dependency_scan_targets(root, &metadata, &mut findings)?;
    scan_package_metadata(&package_scan_targets, &mut findings)?;
    for target in package_scan_targets.values() {
        if is_path_dependency_package(&target.package) {
            let package_root = package_root(&target.package)?;
            scan_path(&package_root, &mut findings)?;
            scan_package_data_prompt_paths(&package_root, &mut findings)?;
        }
    }

    for relative in OPTIONAL_CENTRAL_DATA_PATHS {
        let path = root.join(relative);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                scan_path(&path, &mut findings)?;
                scan_central_data_prompt_markers(&path, &mut findings)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }

    Ok(findings)
}

fn fail_closed_if_required_paths_missing(root: &Path) -> Result<(), String> {
    for relative in CENTRAL_PATHS {
        let path = root.join(relative);
        if !path.exists() {
            return Err(format!(
                "required central path missing for no-inference scan: {}",
                path.display()
            ));
        }
        let manifest = path.join("Cargo.toml");
        if !manifest.exists() {
            return Err(format!(
                "required central manifest missing for no-inference scan: {}",
                manifest.display()
            ));
        }
    }
    Ok(())
}

fn workspace_metadata(root: &Path) -> Result<Metadata, String> {
    MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .exec()
        .map_err(|error| error.to_string())
}

fn central_dependency_scan_targets(
    root: &Path,
    metadata: &Metadata,
    findings: &mut Vec<Finding>,
) -> Result<BTreeMap<PackageId, PackageScanTarget>, String> {
    let mut pending = Vec::new();
    let mut required_package_ids = BTreeSet::<PackageId>::new();
    for (package_name, relative_path) in REQUIRED_CENTRAL_PACKAGES {
        let expected_manifest = root.join(relative_path).join("Cargo.toml");
        let expected_manifest = expected_manifest
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let package = metadata
            .packages
            .iter()
            .find(|package| {
                package
                    .manifest_path
                    .as_std_path()
                    .canonicalize()
                    .map(|manifest_path| manifest_path == expected_manifest)
                    .unwrap_or(false)
            })
            .ok_or_else(|| {
                format!(
                    "required central package missing from expected path: {}",
                    expected_manifest.display()
                )
            })?;
        if package.name != *package_name {
            return Err(format!(
                "central package path mismatch: expected {package_name} at {} but found {}",
                expected_manifest.display(),
                package.name
            ));
        }
        required_package_ids.insert(package.id.clone());
        pending.push((package.id.clone(), true));
    }
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| "cargo metadata did not include dependency resolution".to_owned())?;
    let mut seen = BTreeSet::<PackageId>::new();
    let mut packages = BTreeMap::<PackageId, PackageScanTarget>::new();
    while let Some((package_id, include_dev_dependencies)) = pending.pop() {
        let include_dev_dependencies =
            include_dev_dependencies || required_package_ids.contains(&package_id);
        if !seen.insert(package_id.clone()) {
            continue;
        }
        let package = package_by_id(metadata, &package_id)?.clone();
        let node = resolve
            .nodes
            .iter()
            .find(|node| node.id == package_id)
            .ok_or_else(|| format!("package missing from dependency graph: {package_id}"))?;
        scan_resolved_feature_values(&package, &node.features, findings)?;
        let scan_dependency = is_path_dependency_package(&package)
            || package_contains_forbidden_metadata(&package, include_dev_dependencies);
        if scan_dependency {
            packages.insert(
                package_id.clone(),
                PackageScanTarget {
                    package,
                    include_dev_dependencies,
                },
            );
        }

        pending.extend(
            node.deps
                .iter()
                .filter(|dependency| {
                    include_dev_dependencies
                        || dependency
                            .dep_kinds
                            .iter()
                            .any(|kind| kind.kind != DependencyKind::Development)
                })
                .map(|dependency| (dependency.pkg.clone(), false)),
        );
    }
    Ok(packages)
}

#[derive(Debug, Clone)]
struct PackageScanTarget {
    package: Package,
    include_dev_dependencies: bool,
}

fn package_contains_forbidden_metadata(package: &Package, include_dev_dependencies: bool) -> bool {
    let mut values = vec![
        package.name.to_string(),
        package.id.to_string(),
        package.manifest_path.to_string(),
    ];
    if let Some(source) = &package.source {
        values.push(source.to_string());
    }
    for dependency in package.dependencies.iter().filter(|dependency| {
        include_dev_dependencies || dependency.kind != DependencyKind::Development
    }) {
        values.push(dependency.name.clone());
        values.push(dependency.req.to_string());
        values.extend(dependency.features.iter().cloned());
        if let Some(rename) = &dependency.rename {
            values.push(rename.clone());
        }
    }
    values
        .iter()
        .any(|value| marker_match(value) || forbidden_dependency_name_match(value))
}

fn scan_package_metadata(
    packages: &BTreeMap<PackageId, PackageScanTarget>,
    findings: &mut Vec<Finding>,
) -> Result<(), String> {
    for target in packages.values() {
        let package = &target.package;
        scan_metadata_value(
            &package_root(package)?.join("Cargo.toml"),
            "package metadata",
            &package.name,
            findings,
        );
        scan_dependency_name_value(
            &package_root(package)?.join("Cargo.toml"),
            "package metadata",
            &package.name,
            findings,
        );
        scan_metadata_value(
            &package_root(package)?.join("Cargo.toml"),
            "package metadata",
            &package.id.to_string(),
            findings,
        );
        if let Some(source) = &package.source {
            scan_metadata_value(
                &package_root(package)?.join("Cargo.toml"),
                "package metadata",
                &source.to_string(),
                findings,
            );
        }
        for dependency in package.dependencies.iter().filter(|dependency| {
            target.include_dev_dependencies || dependency.kind != DependencyKind::Development
        }) {
            scan_metadata_value(
                &package_root(package)?.join("Cargo.toml"),
                "dependency metadata",
                &dependency.name,
                findings,
            );
            scan_dependency_name_value(
                &package_root(package)?.join("Cargo.toml"),
                "dependency metadata",
                &dependency.name,
                findings,
            );
            for feature in &dependency.features {
                scan_metadata_value(
                    &package_root(package)?.join("Cargo.toml"),
                    "dependency feature metadata",
                    feature,
                    findings,
                );
                scan_dependency_name_value(
                    &package_root(package)?.join("Cargo.toml"),
                    "dependency feature metadata",
                    feature,
                    findings,
                );
            }
            if let Some(rename) = &dependency.rename {
                scan_metadata_value(
                    &package_root(package)?.join("Cargo.toml"),
                    "dependency metadata",
                    rename,
                    findings,
                );
            }
        }
    }
    Ok(())
}

fn scan_resolved_feature_values(
    package: &Package,
    features: &[FeatureName],
    findings: &mut Vec<Finding>,
) -> Result<(), String> {
    let manifest = package_root(package)?.join("Cargo.toml");
    for feature in features {
        scan_metadata_value(
            &manifest,
            "resolved dependency feature",
            feature.as_ref(),
            findings,
        );
        scan_dependency_name_value(
            &manifest,
            "resolved dependency feature",
            feature.as_ref(),
            findings,
        );
    }
    Ok(())
}

fn scan_metadata_value(
    path: &Path,
    context: &'static str,
    value: &str,
    findings: &mut Vec<Finding>,
) {
    for marker in marker_matches(value) {
        findings.push(Finding {
            path: path.to_path_buf(),
            line: 0,
            marker,
            context,
        });
    }
}

fn scan_central_data_prompt_markers(
    path: &Path,
    findings: &mut Vec<Finding>,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "symlink not allowed in no-inference scan: {}",
            path.display()
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            scan_central_data_prompt_markers(&entry.path(), findings)?;
        }
        return Ok(());
    }
    if !should_scan_file(path) {
        return Ok(());
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    for (index, line) in content.lines().enumerate() {
        let safe_checked_item_removed = line.replace("\"no_arbitrary_prompt\"", "");
        let normalized = normalize_marker_input(&safe_checked_item_removed);
        for marker in CENTRAL_DATA_FORBIDDEN_MARKERS {
            if normalized.contains(&normalize_marker_input(marker.name)) {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    line: index.saturating_add(1),
                    marker: *marker,
                    context: "central schema or migration content",
                });
            }
        }
    }
    Ok(())
}

fn scan_dependency_name_value(
    path: &Path,
    context: &'static str,
    value: &str,
    findings: &mut Vec<Finding>,
) {
    let normalized = normalize_dependency_name(value);
    for forbidden in FORBIDDEN_DEPENDENCY_NAMES {
        if normalized == *forbidden {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: 0,
                marker: ForbiddenMarker::new(forbidden),
                context,
            });
        }
    }
}

fn package_by_id<'a>(
    metadata: &'a Metadata,
    package_id: &PackageId,
) -> Result<&'a Package, String> {
    metadata
        .packages
        .iter()
        .find(|package| package.id == *package_id)
        .ok_or_else(|| format!("package missing from cargo metadata: {package_id}"))
}

fn package_root(package: &Package) -> Result<PathBuf, String> {
    package
        .manifest_path
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| format!("package manifest has no parent: {}", package.manifest_path))
}

fn is_path_dependency_package(package: &Package) -> bool {
    package.source.is_none()
}

fn scan_path(path: &Path, findings: &mut Vec<Finding>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "symlink not allowed in no-inference scan: {}",
            path.display()
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            scan_path(&entry.path(), findings)?;
        }
        return Ok(());
    }

    if should_scan_file(path) {
        scan_file(path, findings)?;
    }
    Ok(())
}

fn should_scan_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SCANNED_EXTENSIONS.contains(&extension))
}

fn scan_file(path: &Path, findings: &mut Vec<Finding>) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    for (index, line) in content.lines().enumerate() {
        for marker in marker_matches(line) {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: index.saturating_add(1),
                marker,
                context: "file content",
            });
        }
        for marker in direct_inference_call_matches(line) {
            findings.push(Finding {
                path: path.to_path_buf(),
                line: index.saturating_add(1),
                marker,
                context: "direct inference call",
            });
        }
    }
    Ok(())
}

fn direct_inference_call_matches(value: &str) -> Vec<ForbiddenMarker> {
    let mut matches = Vec::new();
    for marker in DIRECT_INFERENCE_CALL_MARKERS {
        if contains_call_token(value, marker.name) {
            matches.push(*marker);
        }
    }
    matches
}

fn contains_call_token(value: &str, token: &str) -> bool {
    let mut offset = 0;
    while offset < value.len() {
        let Some((relative_start, _first)) = value[offset..]
            .char_indices()
            .find(|(_, character)| character.is_ascii_alphabetic() || *character == '_')
        else {
            return false;
        };
        let start = offset.saturating_add(relative_start);
        let mut end = start;
        for (relative_index, character) in value[start..].char_indices() {
            if character.is_ascii_alphanumeric() || character == '_' {
                end = start + relative_index + character.len_utf8();
            } else {
                break;
            }
        }
        if &value[start..end] == token {
            let rest = value[end..].trim_start();
            if rest.starts_with('(') {
                return true;
            }
        }
        offset = end;
    }
    false
}

fn scan_package_data_prompt_paths(
    package_root: &Path,
    findings: &mut Vec<Finding>,
) -> Result<(), String> {
    for relative in ["migrations", "schema", "schemas"] {
        let path = package_root.join(relative);
        match fs::symlink_metadata(&path) {
            Ok(_) => scan_central_data_prompt_markers(&path, findings)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn marker_match(value: &str) -> bool {
    !marker_matches(value).is_empty()
}

fn marker_matches(value: &str) -> Vec<ForbiddenMarker> {
    let normalized = normalize_marker_input(value);
    FORBIDDEN_MARKERS
        .iter()
        .copied()
        .filter(|marker| normalized.contains(&normalize_marker_input(marker.name)))
        .collect()
}

fn forbidden_dependency_name_match(value: &str) -> bool {
    let normalized = normalize_dependency_name(value);
    FORBIDDEN_DEPENDENCY_NAMES
        .iter()
        .any(|forbidden| normalized == *forbidden)
}

fn normalize_dependency_name(value: &str) -> String {
    value.replace('_', "-").to_ascii_lowercase()
}

fn normalize_marker_input(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[cfg(unix)]
    #[test]
    fn scan_path_rejects_symlinked_files_or_directories() -> Result<(), Box<dyn Error>> {
        let temp = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("llm.rs"), "call_llm()")?;
        fs::create_dir(temp.path().join("scan"))?;
        std::os::unix::fs::symlink(
            outside.path().join("llm.rs"),
            temp.path().join("scan").join("linked.rs"),
        )?;
        std::os::unix::fs::symlink(outside.path(), temp.path().join("scan").join("linked_dir"))?;

        let mut findings = Vec::new();
        let error = match scan_path(&temp.path().join("scan"), &mut findings) {
            Ok(()) => return Err("symlinked scan path should reject".into()),
            Err(error) => error,
        };

        assert!(error.contains("symlink not allowed in no-inference scan"));
        assert!(findings.is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn scan_path_rejects_symlinked_directory_when_it_is_first_seen() -> Result<(), Box<dyn Error>> {
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("llm.rs"), "call_llm()")?;
        let temp = tempfile::tempdir()?;
        let linked_dir = temp.path().join("linked_dir");
        std::os::unix::fs::symlink(outside.path(), &linked_dir)?;

        let mut findings = Vec::new();
        let error = match scan_path(&linked_dir, &mut findings) {
            Ok(()) => return Err("symlinked scan path should reject".into()),
            Err(error) => error,
        };

        assert!(error.contains("symlink not allowed in no-inference scan"));
        assert!(findings.is_empty());
        Ok(())
    }
}
