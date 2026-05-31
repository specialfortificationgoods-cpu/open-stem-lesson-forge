use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn verifier() -> &'static str {
    env!("CARGO_BIN_EXE_verify-no-leak-fixtures")
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?)
}

#[test]
fn real_workspace_fixtures_are_safe_to_publish() -> Result<(), Box<dyn Error>> {
    let output = Command::new(verifier())
        .arg("--root")
        .arg(workspace_root()?)
        .output()?;

    assert!(
        output.status.success(),
        "expected no-leak fixture verifier to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("no leak markers found"),
        "expected safe success summary"
    );
    Ok(())
}

#[test]
fn unsafe_fixture_values_fail_without_echoing_secret() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let examples = temp.path().join("examples/mvp");
    fs::create_dir_all(&examples)?;
    fs::write(
        examples.join("request.valid.json"),
        r#"{"title":"sk-test-1234567890abcdef","subject":"physics"}"#,
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected unsafe fixture to fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("secret_like_value"),
        "expected safe reason code in stderr"
    );
    assert!(
        !stderr.contains("sk-test-1234567890abcdef"),
        "stderr must not echo unsafe values"
    );
    Ok(())
}

#[test]
fn common_secret_path_and_grade_shapes_fail_safely() -> Result<(), Box<dyn Error>> {
    for (unsafe_value, reason) in [
        ("Bearer abcdef1234567890", "secret_like_value"),
        ("ghp_abcdef1234567890abcdef", "secret_like_value"),
        ("AKIA1234567890ABCDEF", "secret_like_value"),
        (r"D:\Users\teacher\state.json", "local_path_value"),
        ("/opt/private/config.json", "local_path_value"),
        ("/var/tmp/private.txt", "local_path_value"),
        ("Grade record: Jane scored 88", "student_pii_like_value"),
    ] {
        let temp = tempfile::tempdir()?;
        let examples = temp.path().join("examples/mvp");
        fs::create_dir_all(&examples)?;
        fs::write(
            examples.join("request.valid.json"),
            serde_json::json!({"title": unsafe_value}).to_string(),
        )?;

        let output = Command::new(verifier())
            .arg("--root")
            .arg(temp.path())
            .output()?;

        assert!(
            !output.status.success(),
            "expected unsafe fixture to fail for {reason}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(reason), "expected reason {reason}");
        assert!(
            !stderr.contains(unsafe_value),
            "stderr must not echo unsafe value"
        );
    }
    Ok(())
}
