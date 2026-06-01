use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn verifier() -> &'static str {
    env!("CARGO_BIN_EXE_verify-no-inference-core")
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?)
}

#[test]
fn real_workspace_passes_no_inference_scan() -> Result<(), Box<dyn Error>> {
    let output = Command::new(verifier())
        .arg("--root")
        .arg(workspace_root()?)
        .output()?;

    assert!(
        output.status.success(),
        "expected workspace scan to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn central_provider_markers_fail_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    let core = temp.path().join("crates/lessonforge_core/src");
    fs::write(
        core.join("lib.rs"),
        "pub fn bad() { let _ = \"GEMINI_API_KEY\"; call_llm(); semantic_merge(); OpenAI::new(); }",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected forbidden marker scan to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("GEMINI_API_KEY"),
        "stderr should identify the forbidden marker"
    );
    Ok(())
}

#[test]
fn required_provider_credential_markers_fail_scan() -> Result<(), Box<dyn Error>> {
    for marker in [
        "ANTHROPIC_API_KEY",
        "GEMINI_API_KEY",
        "MISTRAL_API_KEY",
        "OPENAI_API_KEY",
        "provider_base_url",
        "model_provider",
    ] {
        let temp = tempfile::tempdir()?;
        write_workspace_manifest(
            temp.path(),
            &[
                "lessonforge_core",
                "lessonforge_api",
                "lessonforge_schema",
                "lessonforge_validator",
            ],
        )?;
        fs::write(
            temp.path().join("crates/lessonforge_api/src/lib.rs"),
            format!("pub const BAD: &str = \"{marker}\";\n"),
        )?;

        let output = Command::new(verifier())
            .arg("--root")
            .arg(temp.path())
            .output()?;

        assert!(
            !output.status.success(),
            "expected marker {marker} to fail\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(marker),
            "stderr should identify marker {marker}"
        );
    }
    Ok(())
}

#[test]
fn direct_inference_entrypoint_names_fail_scan() -> Result<(), Box<dyn Error>> {
    for marker in [
        "call_llm",
        "generate_with_model",
        "embed",
        "infer",
        "plan_with_model",
        "model_moderate",
        "semantic_merge",
    ] {
        let temp = tempfile::tempdir()?;
        write_workspace_manifest(
            temp.path(),
            &[
                "lessonforge_core",
                "lessonforge_api",
                "lessonforge_schema",
                "lessonforge_validator",
            ],
        )?;
        fs::write(
            temp.path().join("crates/lessonforge_core/src/lib.rs"),
            format!("pub fn {marker}() {{}}\n"),
        )?;

        let output = Command::new(verifier())
            .arg("--root")
            .arg(temp.path())
            .output()?;

        assert!(
            !output.status.success(),
            "expected entrypoint {marker} to fail\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(marker),
            "stderr should identify entrypoint {marker}"
        );
    }
    Ok(())
}

#[test]
fn direct_inference_call_sites_fail_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    fs::write(
        temp.path().join("crates/lessonforge_core/src/lib.rs"),
        "pub fn bad() { embed(); infer(); }",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected direct inference call sites to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("embed")
            || String::from_utf8_lossy(&output.stderr).contains("infer"),
        "stderr should identify direct inference call marker"
    );
    Ok(())
}

#[test]
fn provider_markers_are_case_insensitive() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    fs::write(
        temp.path().join("crates/lessonforge_api/src/lib.rs"),
        "pub const BAD: &str = \"OpenAI provider must not be referenced\";\n",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected mixed-case provider marker to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn central_schema_and_migration_fields_fail_scan() -> Result<(), Box<dyn Error>> {
    for (relative, content) in [
        (
            "schemas/request.json",
            r#"{"properties":{"prompt_template":{"type":"string"}}}"#,
        ),
        (
            "schemas/artifact.json",
            r#"{"properties":{"model_prompt":{"type":"string"}}}"#,
        ),
        (
            "schemas/bare-authority.json",
            r#"{"properties":{"prompt":{"type":"string"},"model":{"type":"string"},"provider":{"type":"string"}}}"#,
        ),
        (
            "migrations/001_vectors.sql",
            "CREATE TABLE embedding_vectors (id TEXT PRIMARY KEY);",
        ),
        (
            "migrations/002_prompts.sql",
            "CREATE TABLE prompts (prompt_text TEXT NOT NULL);",
        ),
        (
            "schemas/provider.json",
            r#"{"properties":{"model_provider":{"type":"string"},"provider_base_url":{"type":"string"}}}"#,
        ),
        (
            "schemas/checked_items.json",
            r#"{"enum":["no_arbitrary_raw_prompt_override"]}"#,
        ),
    ] {
        let temp = tempfile::tempdir()?;
        write_workspace_manifest(
            temp.path(),
            &[
                "lessonforge_core",
                "lessonforge_api",
                "lessonforge_schema",
                "lessonforge_validator",
            ],
        )?;
        let target = temp.path().join(relative);
        let parent = target.parent().ok_or("test path must have parent")?;
        fs::create_dir_all(parent)?;
        fs::write(&target, content)?;

        let output = Command::new(verifier())
            .arg("--root")
            .arg(temp.path())
            .output()?;

        assert!(
            !output.status.success(),
            "expected central schema/migration field {relative} to fail\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[test]
fn exact_safe_checked_item_token_does_not_fail_central_schema_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    fs::create_dir_all(temp.path().join("schemas"))?;
    fs::write(
        temp.path().join("schemas/plan_verification.schema.json"),
        r#"{"enum":["no_arbitrary_prompt"],"properties":{"provider_count":{"type":"integer"},"model_state":{"type":"string"}}}"#,
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        output.status.success(),
        "expected exact checked item token to be allowed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn runner_markers_do_not_fail_core_scan_when_not_central_dependencies() -> Result<(), Box<dyn Error>>
{
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "lessonforge_runner",
        ],
    )?;
    fs::write(
        temp.path().join("crates/lessonforge_runner/src/lib.rs"),
        "pub fn local_runner_marker() { let _ = \"OPENAI_API_KEY may exist only in local runner config tests\"; }",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        output.status.success(),
        "expected runner markers outside central dependency graph to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn validator_marker_fails_when_imported_by_central_crate() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    write_dependency(
        temp.path(),
        "lessonforge_api",
        "lessonforge_validator",
        "../lessonforge_validator",
    )?;
    fs::write(
        temp.path().join("crates/lessonforge_validator/src/lib.rs"),
        "pub fn bad() { let _ = \"ollama validator hook\"; }",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected imported validator marker to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("ollama"),
        "stderr should identify imported validator marker"
    );
    Ok(())
}

#[test]
fn provider_env_markers_are_case_insensitive() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    fs::write(
        temp.path().join("crates/lessonforge_api/src/lib.rs"),
        "pub fn bad() { let _ = std::env::var(\"ProviderBaseUrl\"); }",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected case-insensitive provider env marker to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("provider_base_url"),
        "stderr should identify normalized forbidden marker"
    );
    Ok(())
}

#[test]
fn inference_owned_migration_tables_fail_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    fs::create_dir_all(temp.path().join("migrations"))?;
    fs::write(
        temp.path().join("migrations/001_inference.sql"),
        "CREATE TABLE embedding_vectors (id TEXT PRIMARY KEY);",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected inference-owned migration table to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("embedding"),
        "stderr should identify migration marker"
    );
    Ok(())
}

#[test]
fn crate_local_schema_and_migration_prompt_fields_fail_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    fs::create_dir_all(temp.path().join("crates/lessonforge_core/migrations"))?;
    fs::write(
        temp.path()
            .join("crates/lessonforge_core/migrations/001_prompt.sql"),
        "CREATE TABLE prompts (prompt_text TEXT NOT NULL);",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected crate-local prompt migration field to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("prompt"),
        "stderr should identify crate-local prompt field"
    );
    Ok(())
}

#[test]
fn required_central_package_must_live_at_required_path() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core_shadow",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    write_crate(
        &temp.path().join("crates/lessonforge_core_shadow"),
        "lessonforge_core",
    )?;
    fs::create_dir_all(temp.path().join("crates/lessonforge_core/src"))?;
    fs::write(
        temp.path().join("crates/lessonforge_core/src/lib.rs"),
        "pub fn bad() { call_llm(); }",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected central package path mismatch to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("required central manifest missing")
            || String::from_utf8_lossy(&output.stderr).contains("central package path mismatch")
            || String::from_utf8_lossy(&output.stderr).contains("call_llm"),
        "stderr should identify central path mismatch or scan expected central path"
    );
    Ok(())
}

#[test]
fn missing_central_paths_fail_closed() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected missing central paths to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("required central path missing"),
        "stderr should identify missing central paths"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn required_central_manifest_symlink_fails_closed() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let external = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    write_crate(external.path(), "lessonforge_core")?;
    let manifest = temp.path().join("crates/lessonforge_core/Cargo.toml");
    fs::remove_file(&manifest)?;
    std::os::unix::fs::symlink(external.path().join("Cargo.toml"), &manifest)?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected required central manifest symlink to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("required central manifest is a symlink"),
        "stderr should identify the rejected manifest symlink"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn dangling_central_data_symlink_fails_closed() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    std::os::unix::fs::symlink(
        temp.path().join("does-not-exist"),
        temp.path().join("crates/lessonforge_core/schemas"),
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected dangling central data symlink to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("symlink not allowed"),
        "stderr should identify the rejected symlink"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn dangling_root_central_data_symlink_fails_closed() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    std::os::unix::fs::symlink(
        temp.path().join("missing-schemas"),
        temp.path().join("schemas"),
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected dangling root central data symlink to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("symlink not allowed"),
        "stderr should identify the rejected symlink"
    );
    Ok(())
}

#[test]
fn shared_path_dependency_markers_fail_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "shared_policy",
        ],
    )?;
    write_dependency(
        temp.path(),
        "lessonforge_api",
        "shared_policy",
        "../shared_policy",
    )?;
    fs::write(
        temp.path().join("crates/shared_policy/src/lib.rs"),
        "pub fn bad() { generate_with_model(); }",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected shared path dependency marker to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("generate_with_model"),
        "stderr should identify shared dependency marker"
    );
    Ok(())
}

#[test]
fn external_path_dependency_markers_fail_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let external = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
        ],
    )?;
    write_crate(external.path(), "shared_policy")?;
    fs::write(
        external.path().join("src/lib.rs"),
        "pub fn bad() { generate_with_model(); }",
    )?;
    let external_path = external.path().to_string_lossy().into_owned();
    write_dependency(
        temp.path(),
        "lessonforge_api",
        "shared_policy",
        &external_path,
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected external path dependency marker to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("generate_with_model"),
        "stderr should identify external path dependency marker"
    );
    Ok(())
}

#[test]
fn forbidden_dependency_name_fails_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "openai",
        ],
    )?;
    write_dependency(temp.path(), "lessonforge_api", "openai", "../openai")?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected forbidden dependency name to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("openai"),
        "stderr should identify forbidden dependency"
    );
    Ok(())
}

#[test]
fn forbidden_dependency_graph_name_without_text_marker_fails_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "wasmtime",
        ],
    )?;
    write_dependency(temp.path(), "lessonforge_api", "wasmtime", "../wasmtime")?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected forbidden dependency graph package to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wasmtime"),
        "stderr should identify forbidden dependency graph package"
    );
    Ok(())
}

#[test]
fn target_specific_forbidden_dependency_name_fails_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "wasmtime",
        ],
    )?;
    write_target_dependency(temp.path(), "lessonforge_api", "wasmtime", "../wasmtime")?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected target-specific forbidden dependency graph package to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wasmtime"),
        "stderr should identify forbidden dependency graph package"
    );
    Ok(())
}

#[test]
fn transitive_package_dev_dependency_is_not_scanned_as_central_runtime()
-> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "benign_runtime",
            "wasmtime",
        ],
    )?;
    write_dependency(
        temp.path(),
        "lessonforge_api",
        "benign_runtime",
        "../benign_runtime",
    )?;
    write_dev_dependency(temp.path(), "benign_runtime", "wasmtime", "../wasmtime")?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        output.status.success(),
        "expected transitive package dev-dependency to be ignored\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn required_central_package_dev_dependency_is_scanned_even_when_reached_transitively()
-> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "wasmtime",
        ],
    )?;
    write_dependency(
        temp.path(),
        "lessonforge_validator",
        "lessonforge_core",
        "../lessonforge_core",
    )?;
    write_dev_dependency(temp.path(), "lessonforge_core", "wasmtime", "../wasmtime")?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected required central crate dev-dependency to fail even after transitive traversal\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wasmtime"),
        "stderr should identify forbidden central dev-dependency"
    );
    Ok(())
}

#[test]
fn forbidden_dependency_feature_without_text_marker_fails_scan() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    write_workspace_manifest(
        temp.path(),
        &[
            "lessonforge_core",
            "lessonforge_api",
            "lessonforge_schema",
            "lessonforge_validator",
            "benign_runtime",
        ],
    )?;
    write_dependency_with_features(
        temp.path(),
        "lessonforge_api",
        "benign_runtime",
        "../benign_runtime",
        &["wasmtime"],
    )?;
    fs::write(
        temp.path().join("crates/benign_runtime/Cargo.toml"),
        "[package]\nname = \"benign_runtime\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nwasmtime = []\n",
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected forbidden dependency feature to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wasmtime"),
        "stderr should identify forbidden dependency feature"
    );
    Ok(())
}

fn write_workspace_manifest(root: &std::path::Path, crates: &[&str]) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(root.join("crates"))?;
    let members = crates
        .iter()
        .map(|crate_name| format!("\"crates/{crate_name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join("Cargo.toml"),
        format!("[workspace]\nmembers = [{members}]\nresolver = \"3\"\n"),
    )?;
    for crate_name in crates {
        write_crate(&root.join(format!("crates/{crate_name}")), crate_name)?;
    }
    Ok(())
}

fn write_crate(root: &std::path::Path, crate_name: &str) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    )?;
    fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n")?;
    Ok(())
}

fn write_dependency(
    root: &std::path::Path,
    crate_name: &str,
    dependency_name: &str,
    dependency_path: &str,
) -> Result<(), Box<dyn Error>> {
    fs::write(
        root.join(format!("crates/{crate_name}/Cargo.toml")),
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependency_name} = {{ path = \"{dependency_path}\" }}\n"
        ),
    )?;
    Ok(())
}

fn write_dependency_with_features(
    root: &std::path::Path,
    crate_name: &str,
    dependency_name: &str,
    dependency_path: &str,
    features: &[&str],
) -> Result<(), Box<dyn Error>> {
    let feature_list = features
        .iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join(format!("crates/{crate_name}/Cargo.toml")),
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependency_name} = {{ path = \"{dependency_path}\", features = [{feature_list}] }}\n"
        ),
    )?;
    Ok(())
}

fn write_dev_dependency(
    root: &std::path::Path,
    crate_name: &str,
    dependency_name: &str,
    dependency_path: &str,
) -> Result<(), Box<dyn Error>> {
    fs::write(
        root.join(format!("crates/{crate_name}/Cargo.toml")),
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dev-dependencies]\n{dependency_name} = {{ path = \"{dependency_path}\" }}\n"
        ),
    )?;
    Ok(())
}

fn write_target_dependency(
    root: &std::path::Path,
    crate_name: &str,
    dependency_name: &str,
    dependency_path: &str,
) -> Result<(), Box<dyn Error>> {
    fs::write(
        root.join(format!("crates/{crate_name}/Cargo.toml")),
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[target.'cfg(any())'.dependencies]\n{dependency_name} = {{ path = \"{dependency_path}\" }}\n"
        ),
    )?;
    Ok(())
}
