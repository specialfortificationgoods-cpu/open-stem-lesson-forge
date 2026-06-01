use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

fn e2e() -> &'static str {
    env!("CARGO_BIN_EXE_lessonforge-e2e")
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?)
}

#[test]
fn mvp_suite_passes_and_reports_required_rows() -> Result<(), Box<dyn Error>> {
    let output = Command::new(e2e())
        .arg("--suite")
        .arg("mvp")
        .arg("--root")
        .arg(workspace_root()?)
        .output()?;

    assert!(
        output.status.success(),
        "expected MVP suite to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("E2E-001"));
    assert!(stdout.contains("CR-GATE-004"));
    assert!(stdout.contains("API-GATE-002"));
    assert!(stdout.contains("E2E-001 executed_smoke"));
    assert!(stdout.contains("E2E-002 executed_smoke"));
    assert!(stdout.contains("LEAK-004 executed/passed negative_unavailable_verified"));
    assert!(stdout.contains("API-GATE-002 executed/passed negative_unavailable_verified"));
    assert!(stdout.contains("mvp suite passed"));
    Ok(())
}

#[test]
fn single_case_runs_from_manifest() -> Result<(), Box<dyn Error>> {
    let output = Command::new(e2e())
        .arg("--suite")
        .arg("mvp")
        .arg("--case")
        .arg("LEAK-004")
        .arg("--root")
        .arg(workspace_root()?)
        .output()?;

    assert!(
        output.status.success(),
        "expected deferred negative case to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("LEAK-004"));
    assert!(stdout.contains("executed/passed via"));
    Ok(())
}

#[test]
fn single_delegated_case_is_not_reported_as_executed() -> Result<(), Box<dyn Error>> {
    let output = Command::new(e2e())
        .arg("--suite")
        .arg("mvp")
        .arg("--case")
        .arg("CR-GATE-004")
        .arg("--root")
        .arg(workspace_root()?)
        .output()?;

    assert!(
        output.status.success(),
        "expected delegated manifest case to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CR-GATE-004"));
    assert!(stdout.contains("listed/delegated via"));
    assert!(!stdout.contains("passed via"));
    Ok(())
}

#[test]
fn unavailable_transport_cases_are_negative_tests() -> Result<(), Box<dyn Error>> {
    for case_id in ["LEAK-004", "API-GATE-002"] {
        let output = Command::new(e2e())
            .arg("--suite")
            .arg("mvp")
            .arg("--case")
            .arg(case_id)
            .arg("--root")
            .arg(workspace_root()?)
            .output()?;

        assert!(
            output.status.success(),
            "expected unavailable case {case_id} to pass as a negative test\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(case_id));
        assert!(stdout.contains("negative_unavailable_verified"));
    }
    Ok(())
}

#[test]
fn unknown_case_fails_with_safe_error() -> Result<(), Box<dyn Error>> {
    let output = Command::new(e2e())
        .arg("--suite")
        .arg("mvp")
        .arg("--case")
        .arg("UNKNOWN-999")
        .arg("--root")
        .arg(workspace_root()?)
        .output()?;

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown_case"));
    Ok(())
}
