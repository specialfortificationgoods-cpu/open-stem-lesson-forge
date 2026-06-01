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
fn uppercase_json_extension_is_scanned_as_json() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let examples = temp.path().join("examples/mvp");
    fs::create_dir_all(&examples)?;
    fs::write(
        examples.join("request.valid.JSON"),
        serde_json::json!({"title": "Safe conservation lesson"}).to_string(),
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        output.status.success(),
        "expected uppercase JSON extension to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
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
        ("api_key=redacted", "secret_like_value"),
        ("access_token: redacted", "secret_like_value"),
        ("token: abcdef1234567890", "secret_like_value"),
        ("secret=abcdef1234567890", "secret_like_value"),
        ("ghp_abcdef1234567890abcdef", "secret_like_value"),
        ("AKIA1234567890ABCDEF", "secret_like_value"),
        (r"D:\Users\teacher\state.json", "local_path_value"),
        ("/opt/private/config.json", "local_path_value"),
        ("/var/tmp/private.txt", "local_path_value"),
        ("teacher@example.test", "student_pii_like_value"),
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
        let escaped_unsafe_value = unsafe_value.replace('\\', "\\\\");
        assert!(
            !stderr.contains(unsafe_value) && !stderr.contains(&escaped_unsafe_value),
            "stderr must not echo unsafe value in raw or escaped form"
        );
    }
    Ok(())
}

#[test]
fn invalid_json_reports_path_and_continues_scanning() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let examples = temp.path().join("examples/mvp");
    fs::create_dir_all(&examples)?;
    fs::write(examples.join("bad.valid.json"), "{ not json")?;
    fs::write(
        examples.join("unsafe.valid.json"),
        serde_json::json!({"title": "Bearer abcdef1234567890"}).to_string(),
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected invalid JSON and unsafe fixture to fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("bad.valid.json:fixture_invalid_json"),
        "expected invalid JSON path in stderr, got {stderr}"
    );
    assert!(
        stderr.contains("unsafe.valid.json:secret_like_value"),
        "expected scanner to continue after invalid JSON, got {stderr}"
    );
    assert!(
        !stderr.contains("abcdef1234567890"),
        "stderr must not echo unsafe values"
    );
    Ok(())
}

#[test]
fn unreadable_json_reports_path_and_continues_scanning() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let examples = temp.path().join("examples/mvp");
    fs::create_dir_all(&examples)?;
    let unreadable = examples.join("unreadable.valid.json");
    fs::write(&unreadable, [0xff])?;
    fs::write(
        examples.join("unsafe.valid.json"),
        serde_json::json!({"title": "Bearer abcdef1234567890"}).to_string(),
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "expected unreadable and unsafe fixtures to fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unreadable.valid.json:fixture_unreadable"),
        "expected unreadable fixture path in stderr, got {stderr}"
    );
    assert!(
        stderr.contains("unsafe.valid.json:secret_like_value"),
        "expected scanner to continue after unreadable fixture, got {stderr}"
    );
    assert!(
        !stderr.contains("abcdef1234567890"),
        "stderr must not echo unsafe values"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn unreadable_directory_reports_path_and_continues_scanning() -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::PermissionsExt as _;

    let uid_output = Command::new("id").arg("-u").output()?;
    if uid_output.status.success() && String::from_utf8_lossy(&uid_output.stdout).trim() == "0" {
        eprintln!("skipping unreadable directory check for root user");
        return Ok(());
    }

    let temp = tempfile::tempdir()?;
    let examples = temp.path().join("examples/mvp");
    fs::create_dir_all(&examples)?;
    let unreadable = examples.join("aaa-unreadable");
    fs::create_dir_all(&unreadable)?;
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000))?;
    fs::write(
        examples.join("unsafe.valid.json"),
        serde_json::json!({"title": "Bearer abcdef1234567890"}).to_string(),
    )?;

    let output_result = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output();
    let restore_result = fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o700));
    let output = output_result?;
    restore_result?;

    assert!(
        !output.status.success(),
        "expected unreadable directory and unsafe fixture to fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("aaa-unreadable:fixture_unreadable"),
        "expected unreadable directory path in stderr, got {stderr}"
    );
    assert!(
        stderr.contains("unsafe.valid.json:secret_like_value"),
        "expected scanner to continue after unreadable directory, got {stderr}"
    );
    assert!(
        !stderr.contains("abcdef1234567890"),
        "stderr must not echo unsafe values"
    );
    Ok(())
}

#[test]
fn benign_at_symbols_and_plain_words_are_not_secret_or_pii() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let examples = temp.path().join("examples/mvp");
    fs::create_dir_all(&examples)?;
    fs::write(
        examples.join("request.valid.json"),
        serde_json::json!({
            "title": "Torque at @symbol notation",
            "notes": "Secret as a vocabulary word, token as grammar, and cookie as classroom analogy.",
            "compiler": "Compiler lesson vocabulary token: identifier and literal. Secret: a hidden value in a word problem."
        })
        .to_string(),
    )?;

    let output = Command::new(verifier())
        .arg("--root")
        .arg(temp.path())
        .output()?;

    assert!(
        output.status.success(),
        "expected benign fixture text to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
