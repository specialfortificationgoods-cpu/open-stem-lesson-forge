use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    path: PathBuf,
    reason: &'static str,
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
    let root = parse_root(env::args().skip(1).collect())?;
    let findings = scan_root(&root)?;
    if findings.is_empty() {
        return Ok("no leak markers found in MVP fixtures".to_owned());
    }

    for finding in findings {
        eprintln!("{}:{}", finding.path.display(), finding.reason);
    }
    Err("no-leak fixture guardrail failed".to_owned())
}

fn parse_root(args: Vec<String>) -> Result<PathBuf, String> {
    match args.as_slice() {
        [] => env::current_dir().map_err(|error| error.to_string()),
        [flag, root] if flag == "--root" => Ok(PathBuf::from(root)),
        _ => Err("usage: verify-no-leak-fixtures [--root PATH]".to_owned()),
    }
}

fn scan_root(root: &Path) -> Result<Vec<Finding>, String> {
    let fixture_root = root.join("examples").join("mvp");
    if !fixture_root.exists() {
        return Err(format!(
            "{}:missing_fixture_directory",
            fixture_root.display()
        ));
    }
    let mut findings = Vec::new();
    scan_path(&fixture_root, &mut findings)?;
    Ok(findings)
}

fn scan_path(path: &Path, findings: &mut Vec<Finding>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            scan_path(&entry.path(), findings)?;
        }
        return Ok(());
    }

    if !metadata.is_file() {
        findings.push(Finding {
            path: path.to_path_buf(),
            reason: "unsupported_fixture_file_type",
        });
        return Ok(());
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
        findings.push(Finding {
            path: path.to_path_buf(),
            reason: "unsupported_fixture_extension",
        });
        return Ok(());
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let json = serde_json::from_str::<Value>(&content).map_err(|_| "fixture_invalid_json")?;
    scan_json_strings(path, &json, findings);
    Ok(())
}

fn scan_json_strings(path: &Path, value: &Value, findings: &mut Vec<Finding>) {
    match value {
        Value::String(text) => {
            for reason in unsafe_reasons(text) {
                findings.push(Finding {
                    path: path.to_path_buf(),
                    reason,
                });
            }
        }
        Value::Array(values) => {
            for child in values {
                scan_json_strings(path, child, findings);
            }
        }
        Value::Object(object) => {
            for child in object.values() {
                scan_json_strings(path, child, findings);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn unsafe_reasons(text: &str) -> Vec<&'static str> {
    let lower = text.to_ascii_lowercase();
    let mut reasons = Vec::new();
    if lower.contains("sk-")
        || lower.contains("bearer ")
        || lower.contains("ghp_")
        || lower.contains("akia")
        || lower.contains("api_key")
        || lower.contains("password")
        || lower.contains("credential=")
        || lower.contains("cookie")
        || lower.contains("oauth")
        || lower.contains("token:")
        || lower.contains("secret")
    {
        reasons.push("secret_like_value");
    }
    if lower.contains("/users/")
        || lower.contains("/home/")
        || lower.contains("/tmp/")
        || lower.contains("/etc/")
        || lower.contains("/opt/")
        || lower.contains("/var/")
        || lower.contains("~/")
        || lower.contains("\\users\\")
        || lower.contains("c:\\")
        || lower.contains("d:\\")
        || lower.contains(".ssh")
        || lower.contains(".codex")
    {
        reasons.push("local_path_value");
    }
    if lower.contains("http://") || lower.contains("https://") || lower.contains("file://") {
        reasons.push("unsafe_url_value");
    }
    if lower.contains('@')
        || lower.contains("student roster")
        || lower.contains("student alice")
        || lower.contains("grade record")
        || lower.contains("scored")
        || lower.contains("alice scored")
        || lower.contains("bob scored")
        || lower.contains("phone")
        || lower.contains('%')
        || contains_grade_fraction(text)
        || contains_titlecase_name_pair(text)
        || has_phone_like_digit_run(text)
    {
        reasons.push("student_pii_like_value");
    }
    reasons.sort_unstable();
    reasons.dedup();
    reasons
}

fn has_phone_like_digit_run(value: &str) -> bool {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .take(7)
        .count()
        >= 7
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

fn contains_grade_fraction(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    chars
        .windows(3)
        .any(|window| window[0].is_ascii_digit() && window[1] == '/' && window[2].is_ascii_digit())
}

fn titlecase_word_shape(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    word.len() >= 2 && first.is_ascii_uppercase() && chars.all(|ch| ch.is_ascii_lowercase())
}
