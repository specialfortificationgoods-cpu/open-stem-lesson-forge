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
    let json = match serde_json::from_str::<Value>(&content) {
        Ok(json) => json,
        Err(_) => {
            findings.push(Finding {
                path: path.to_path_buf(),
                reason: "fixture_invalid_json",
            });
            return Ok(());
        }
    };
    scan_json_strings(path, &json, findings);
    Ok(())
}

fn scan_json_strings(path: &Path, value: &Value, findings: &mut Vec<Finding>) {
    match value {
        Value::String(text) => scan_text(path, text, findings),
        Value::Array(values) => {
            for child in values {
                scan_json_strings(path, child, findings);
            }
        }
        Value::Object(object) => {
            for (key, child) in object {
                scan_key(path, key, findings);
                scan_json_strings(path, child, findings);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn scan_text(path: &Path, text: &str, findings: &mut Vec<Finding>) {
    for reason in unsafe_reasons(text) {
        findings.push(Finding {
            path: path.to_path_buf(),
            reason,
        });
    }
}

fn scan_key(path: &Path, key: &str, findings: &mut Vec<Finding>) {
    let lower = key.to_ascii_lowercase();
    if contains_secret_like_key(&lower) {
        findings.push(Finding {
            path: path.to_path_buf(),
            reason: "secret_like_value",
        });
    }
    scan_text(path, key, findings);
}

fn unsafe_reasons(text: &str) -> Vec<&'static str> {
    let lower = text.to_ascii_lowercase();
    let mut reasons = Vec::new();
    if contains_secret_like_value(&lower) {
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
    if contains_email_like_value(text)
        || lower.contains("student roster")
        || lower.contains("student alice")
        || lower.contains("grade record")
        || lower.contains("scored")
        || lower.contains("alice scored")
        || lower.contains("bob scored")
        || contains_contextual_percent_grade(text, &lower)
        || contains_contextual_grade_fraction(text, &lower)
        || contains_contextual_titlecase_name_pair(text)
        || has_phone_like_digit_run(text, &lower)
        || contains_separated_nine_digit_identifier(text)
        || contains_contextual_numeric_identifier(text, &lower)
    {
        reasons.push("student_pii_like_value");
    }
    reasons.sort_unstable();
    reasons.dedup();
    reasons
}

fn contains_secret_like_key(lower: &str) -> bool {
    let tokens = lower
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return false;
    }
    tokens.iter().any(|token| {
        matches!(
            *token,
            "apikey" | "password" | "credential" | "credentials" | "cookie" | "cookies" | "oauth"
        )
    }) || contains_token_sequence(&tokens, &["api", "key"])
        || contains_token_sequence(&tokens, &["access", "token"])
        || contains_token_sequence(&tokens, &["refresh", "token"])
        || contains_token_sequence(&tokens, &["auth", "token"])
        || contains_token_sequence(&tokens, &["bearer", "token"])
        || contains_token_sequence(&tokens, &["client", "secret"])
        || contains_token_sequence(&tokens, &["api", "secret"])
        || key_has_sensitive_suffix(&tokens)
}

fn contains_token_sequence(tokens: &[&str], sequence: &[&str]) -> bool {
    tokens
        .windows(sequence.len())
        .any(|window| window == sequence)
}

fn key_has_sensitive_suffix(tokens: &[&str]) -> bool {
    tokens
        .last()
        .is_some_and(|last| matches!(*last, "token" | "tokens" | "secret" | "secrets"))
}

fn contains_secret_like_value(lower: &str) -> bool {
    if contains_known_secret_marker(lower) {
        return true;
    }
    [
        "api_key",
        "apikey",
        "access_token",
        "refresh_token",
        "auth_token",
        "bearer_token",
        "client_secret",
        "api_secret",
        "password",
        "credential",
        "credentials",
        "cookie",
        "cookies",
        "oauth",
    ]
    .iter()
    .any(|alias| contains_assignment_like_alias(lower, alias))
        || ["token", "tokens", "secret", "secrets"]
            .iter()
            .any(|alias| contains_assignment_like_secret_value(lower, alias))
}

fn contains_assignment_like_alias(lower: &str, alias: &str) -> bool {
    assignment_like_values(lower, alias).next().is_some()
}

fn contains_assignment_like_secret_value(lower: &str, alias: &str) -> bool {
    assignment_like_values(lower, alias).any(assigned_value_looks_secret)
}

fn assignment_like_values<'a>(lower: &'a str, alias: &'a str) -> impl Iterator<Item = &'a str> {
    let mut search_start = 0;
    std::iter::from_fn(move || {
        while let Some(relative_index) = lower[search_start..].find(alias) {
            let index = search_start + relative_index;
            let before_ok = lower[..index]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
            let after_alias = index + alias.len();
            let suffix = &lower[after_alias..];
            let after_ok = suffix
                .chars()
                .next()
                .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
            search_start = after_alias;
            if !before_ok || !after_ok {
                continue;
            }
            let trimmed_suffix =
                suffix.trim_start_matches(|character: char| character.is_ascii_whitespace());
            if let Some(separator) = trimmed_suffix
                .chars()
                .next()
                .filter(|character| matches!(character, ':' | '='))
            {
                return Some(&trimmed_suffix[separator.len_utf8()..]);
            }
        }
        None
    })
}

fn assigned_value_looks_secret(value: &str) -> bool {
    let trimmed = value.trim_start();
    if contains_known_secret_marker(trimmed) {
        return true;
    }
    let first_token = trimmed
        .split(|character: char| character.is_ascii_whitespace() || matches!(character, ',' | ';'))
        .next()
        .unwrap_or_default()
        .trim_matches(|character: char| matches!(character, '"' | '\'' | '[' | ']' | '{' | '}'));
    first_token.len() >= 12
        && first_token.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | '-' | '.' | '/' | '+' | '=')
        })
}

fn contains_known_secret_marker(lower: &str) -> bool {
    contains_delimited_prefix(lower, "sk-")
        || contains_sk_credential_marker(lower)
        || lower.contains("bearer ")
        || lower.contains("authorization: bearer")
        || contains_delimited_prefix(lower, "ghp_")
        || contains_delimited_prefix(lower, "akia")
}

fn contains_sk_credential_marker(lower: &str) -> bool {
    ["sk_live", "sk_test", "sk_proj"]
        .iter()
        .any(|prefix| contains_delimited_prefix(lower, prefix))
}

fn contains_delimited_prefix(lower: &str, prefix: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative_index) = lower[search_start..].find(prefix) {
        let index = search_start + relative_index;
        let before_ok = lower[..index]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
        if before_ok {
            return true;
        }
        search_start = index + prefix.len();
    }
    false
}

fn contains_email_like_value(value: &str) -> bool {
    value
        .match_indices('@')
        .any(|(at_index, _)| email_like_at(value, at_index))
}

fn email_like_at(value: &str, at_index: usize) -> bool {
    let local = email_local_part_before(value, at_index);
    let domain = email_domain_part_after(value, at_index + 1);
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return false;
    }
    domain.rsplit('.').next().is_some_and(|tld| {
        tld.len() >= 2 && tld.chars().all(|character| character.is_ascii_alphabetic())
    })
}

fn email_local_part_before(value: &str, at_index: usize) -> &str {
    let local_start = value[..at_index]
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!is_email_local_character(character)).then_some(index + character.len_utf8())
        })
        .unwrap_or(0);
    &value[local_start..at_index]
}

fn email_domain_part_after(value: &str, domain_start: usize) -> &str {
    let domain_end = value[domain_start..]
        .char_indices()
        .find_map(|(index, character)| {
            (!is_email_domain_character(character)).then_some(domain_start + index)
        })
        .unwrap_or(value.len());
    &value[domain_start..domain_end]
}

fn is_email_local_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '%' | '+' | '-')
}

fn is_email_domain_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '.' | '-')
}

fn has_phone_like_digit_run(value: &str, lower: &str) -> bool {
    if contains_formatted_phone_number(value) {
        return true;
    }
    if contains_space_grouped_phone_number(value) {
        return true;
    }
    if contains_international_phone_number(value) {
        return true;
    }
    if contains_contextual_phone_number(value, lower) {
        return true;
    }
    if contains_contextual_contiguous_phone_number(value, lower) {
        return true;
    }
    false
}

fn contains_contextual_contiguous_phone_number(value: &str, lower: &str) -> bool {
    contiguous_digit_run_spans(value)
        .into_iter()
        .any(|span| matches!(span.len, 10 | 11) && digit_run_has_phone_context(lower, span))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DigitRunSpan {
    start: usize,
    end: usize,
    len: usize,
}

fn contiguous_digit_run_spans(value: &str) -> Vec<DigitRunSpan> {
    let mut spans = Vec::new();
    let mut run_start = None;
    let mut run_len = 0;
    for (index, character) in value
        .char_indices()
        .chain(std::iter::once((value.len(), '\0')))
    {
        if character.is_ascii_digit() {
            if run_start.is_none() {
                run_start = Some(index);
            }
            run_len += 1;
        } else if run_len > 0 {
            spans.push(DigitRunSpan {
                start: run_start.unwrap_or(index),
                end: index,
                len: run_len,
            });
            run_start = None;
            run_len = 0;
        }
    }
    spans
}

fn digit_run_has_phone_context(lower: &str, span: DigitRunSpan) -> bool {
    phone_context_before_candidate(lower, span.start)
        || phone_context_after_candidate(lower, span.end)
}

fn contains_formatted_phone_number(value: &str) -> bool {
    let mut groups = Vec::new();
    let mut current_group_len = 0;
    let mut candidate_started = false;
    let mut saw_phone_punctuation = false;
    for character in value.chars().chain(std::iter::once('\0')) {
        if character.is_ascii_digit() {
            current_group_len += 1;
            candidate_started = true;
            continue;
        }
        if candidate_started && matches!(character, ' ' | '-' | '.' | '(' | ')') {
            if current_group_len > 0 {
                groups.push(current_group_len);
                current_group_len = 0;
            }
            saw_phone_punctuation |= matches!(character, '-' | '.' | '(' | ')');
            continue;
        }
        if current_group_len > 0 {
            groups.push(current_group_len);
            current_group_len = 0;
        }
        if saw_phone_punctuation && groups_match_space_grouped_phone(&groups) {
            return true;
        }
        groups.clear();
        candidate_started = false;
        saw_phone_punctuation = false;
    }
    saw_phone_punctuation && groups_match_space_grouped_phone(&groups)
}

fn contains_space_grouped_phone_number(value: &str) -> bool {
    let mut groups = Vec::new();
    let mut current_group_len = 0;
    let mut candidate_started = false;
    for character in value.chars().chain(std::iter::once('\0')) {
        if character.is_ascii_digit() {
            current_group_len += 1;
            candidate_started = true;
            continue;
        }
        if candidate_started && character.is_ascii_whitespace() {
            if current_group_len > 0 {
                groups.push(current_group_len);
                current_group_len = 0;
            }
            continue;
        }
        if current_group_len > 0 {
            groups.push(current_group_len);
            current_group_len = 0;
        }
        if groups_match_space_grouped_phone(&groups) {
            return true;
        }
        groups.clear();
        candidate_started = false;
    }
    groups_match_space_grouped_phone(&groups)
}

fn groups_match_space_grouped_phone(groups: &[usize]) -> bool {
    matches!(groups, [3, 3, 4] | [1, 3, 3, 4])
}

fn contains_contextual_phone_number(value: &str, lower: &str) -> bool {
    for (start, character) in value.char_indices() {
        if !character.is_ascii_digit() && character != '(' {
            continue;
        }
        if previous_char(value, start).is_some_and(|previous| previous.is_ascii_digit()) {
            continue;
        }
        let candidate = &value[start..];
        let end = phone_candidate_end(candidate);
        if end == 0 {
            continue;
        }
        let context_before = phone_context_before_candidate(lower, start);
        let context_after = phone_context_after_candidate(lower, start + end);
        if !context_before && !context_after {
            continue;
        }
        let groups = digit_group_lengths(&candidate[..end]);
        if matches!(groups.as_slice(), [7] | [3, 4] | [3, 3, 4] | [1, 3, 3, 4])
            || groups_match_contextual_international_phone(&groups)
        {
            return true;
        }
    }
    false
}

fn contains_international_phone_number(value: &str) -> bool {
    value.match_indices('+').any(|(plus_index, _)| {
        if previous_char(value, plus_index).is_some_and(|previous| previous.is_ascii_digit()) {
            return false;
        }
        international_phone_number_at(value, plus_index)
    })
}

fn international_phone_number_at(value: &str, plus_index: usize) -> bool {
    let mut groups = Vec::new();
    let mut current_group_len = 0;
    for character in value[plus_index + 1..].chars().chain(std::iter::once('\0')) {
        if character.is_ascii_digit() {
            current_group_len += 1;
            continue;
        }
        if matches!(character, ' ' | '-' | '.' | '(' | ')') {
            if current_group_len > 0 {
                groups.push(current_group_len);
                current_group_len = 0;
            }
            continue;
        }
        if current_group_len > 0 {
            groups.push(current_group_len);
        }
        break;
    }
    groups_match_international_phone(&groups)
}

fn groups_match_international_phone(groups: &[usize]) -> bool {
    let digit_count: usize = groups.iter().copied().sum();
    if !(8..=15).contains(&digit_count) {
        return false;
    }
    groups.len() == 1 || (groups.len() >= 3 && (1..=3).contains(&groups[0]))
}

fn groups_match_contextual_international_phone(groups: &[usize]) -> bool {
    let digit_count: usize = groups.iter().copied().sum();
    (8..=15).contains(&digit_count)
        && (groups.len() == 1 || groups.len() >= 3 || matches!(groups, [4, 4, 4]))
}

fn phone_context_before_candidate(lowercase: &str, candidate_start: usize) -> bool {
    let prefix = lowercase[..candidate_start].trim_end_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.' | '(')
    });
    let mut words = prefix
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    while words
        .last()
        .is_some_and(|word| matches!(*word, "is" | "at"))
    {
        words.pop();
    }
    let contact_suffixes: &[&[&str]] = &[
        &["phone"],
        &["phone", "number"],
        &["mobile"],
        &["mobile", "number"],
        &["cell", "phone", "number"],
        &["text"],
        &["text", "me"],
        &["text", "me", "at"],
        &["sms"],
        &["sms", "at"],
        &["call"],
        &["call", "me"],
        &["call", "me", "at"],
        &["contact"],
        &["contact", "me"],
        &["contact", "me", "at"],
        &["contact", "phone"],
        &["tel"],
    ];
    contact_suffixes
        .iter()
        .any(|suffix| contact_suffix_matches(&words, suffix))
}

fn contact_suffix_matches(words: &[&str], suffix: &[&str]) -> bool {
    if !words.ends_with(suffix) {
        return false;
    }
    suffix != ["phone"] || !words.ends_with(&["cell", "phone"])
}

fn phone_context_after_candidate(lowercase: &str, candidate_end: usize) -> bool {
    let suffix = lowercase[candidate_end..].trim_start_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.' | ')')
    });
    let words = suffix
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(3)
        .collect::<Vec<_>>();
    let contact_prefixes: &[&[&str]] = &[
        &["phone"],
        &["phone", "number"],
        &["mobile"],
        &["mobile", "number"],
        &["cell", "phone", "number"],
        &["tel"],
    ];
    contact_prefixes
        .iter()
        .any(|prefix| words.starts_with(prefix))
}

fn phone_candidate_end(candidate: &str) -> usize {
    candidate
        .char_indices()
        .find_map(|(index, character)| (!is_phone_base_character(character)).then_some(index))
        .unwrap_or(candidate.len())
}

fn is_phone_base_character(character: char) -> bool {
    character.is_ascii_digit() || matches!(character, ' ' | '-' | '.' | '(' | ')')
}

fn digit_group_lengths(base: &str) -> Vec<usize> {
    base.split(|character: char| !character.is_ascii_digit())
        .filter(|group| !group.is_empty())
        .map(str::len)
        .collect()
}

fn previous_char(value: &str, index: usize) -> Option<char> {
    value[..index].chars().next_back()
}

fn contains_contextual_percent_grade(value: &str, lower: &str) -> bool {
    let has_grade_context = [
        "attendance",
        "grade",
        "score",
        "scored",
        "quiz",
        "test",
        "exam",
        "student",
        "learner",
        "roster",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    has_grade_context && contains_digit_percent_pair(value)
}

fn contains_digit_percent_pair(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    for (index, character) in chars.iter().enumerate() {
        if *character == '%' {
            let previous_digit = chars[..index]
                .iter()
                .rev()
                .find(|candidate| !candidate.is_ascii_whitespace())
                .is_some_and(|candidate| candidate.is_ascii_digit());
            let next_digit = chars[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_ascii_whitespace())
                .is_some_and(|candidate| candidate.is_ascii_digit());
            if previous_digit || next_digit {
                return true;
            }
        }
    }
    false
}

fn contains_contextual_grade_fraction(value: &str, lower: &str) -> bool {
    let has_grade_context = [
        "attendance",
        "grade",
        "score",
        "scored",
        "quiz",
        "test",
        "exam",
        "student",
        "learner",
        "roster",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    has_grade_context && contains_grade_fraction(value)
}

fn contains_grade_fraction(value: &str) -> bool {
    let chars = value.chars().collect::<Vec<_>>();
    chars
        .windows(3)
        .any(|window| window[0].is_ascii_digit() && window[1] == '/' && window[2].is_ascii_digit())
}

fn contains_separated_nine_digit_identifier(value: &str) -> bool {
    let mut groups = Vec::new();
    let mut current_group_len = 0;
    let mut candidate_started = false;
    let mut saw_separator = false;
    for character in value.chars().chain(std::iter::once('\0')) {
        if character.is_ascii_digit() {
            current_group_len += 1;
            candidate_started = true;
            continue;
        }
        if candidate_started && matches!(character, '-' | ' ' | '\t') {
            if current_group_len > 0 {
                groups.push(current_group_len);
                current_group_len = 0;
            }
            saw_separator = true;
            continue;
        }
        if current_group_len > 0 {
            groups.push(current_group_len);
            current_group_len = 0;
        }
        if saw_separator && groups == [3, 2, 4] {
            return true;
        }
        groups.clear();
        candidate_started = false;
        saw_separator = false;
    }
    saw_separator && groups == [3, 2, 4]
}

fn contains_contextual_numeric_identifier(value: &str, lower: &str) -> bool {
    let normalized_label_text = normalize_identifier_context(lower);
    let has_identifier_context = [
        "student id",
        "student identifier",
        "student number",
        "learner id",
        "learner number",
        "pupil id",
        "pupil number",
        "ssn",
        "social security",
    ]
    .iter()
    .any(|marker| normalized_label_text.contains(marker));
    has_identifier_context
        && value
            .chars()
            .filter(|character| character.is_ascii_digit())
            .count()
            >= 5
}

fn normalize_identifier_context(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_was_space = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            previous_was_space = false;
        } else if matches!(character, '-' | '_' | ':' | ' ' | '\t') && !previous_was_space {
            output.push(' ');
            previous_was_space = true;
        }
    }
    output
}

fn contains_contextual_titlecase_name_pair(value: &str) -> bool {
    let words = value
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let lowered_words = words
        .iter()
        .map(|word| word.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let context_markers: &[&[&str]] = &[
        &["student"],
        &["learner"],
        &["roster"],
        &["attendance"],
        &["grade", "record"],
        &["class", "list"],
    ];
    context_markers.iter().any(|marker| {
        lowered_words
            .windows(marker.len())
            .enumerate()
            .any(|(index, window)| {
                window.iter().map(String::as_str).eq(marker.iter().copied()) && {
                    let mut name_start = index + marker.len();
                    while lowered_words.get(name_start).is_some_and(|word| {
                        matches!(word.as_str(), "entry" | "name" | "full" | "legal")
                    }) {
                        name_start += 1;
                    }
                    contains_titlecase_name_sequence(&words[name_start..])
                }
            })
    })
}

fn contains_titlecase_name_sequence(words: &[&str]) -> bool {
    match words {
        [first, middle, last, ..]
            if titlecase_word_shape(first)
                && middle.len() == 1
                && middle
                    .chars()
                    .all(|character| character.is_ascii_uppercase())
                && titlecase_word_shape(last) =>
        {
            true
        }
        [first, last, ..] => titlecase_word_shape(first) && titlecase_word_shape(last),
        _ => false,
    }
}

fn titlecase_word_shape(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    word.len() >= 2 && first.is_ascii_uppercase() && chars.all(|ch| ch.is_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_stem_text_is_not_student_pii_like() {
        for text in [
            "Newton Second Law",
            "Use the percent symbol % in a worksheet",
            "Calculate 85% of 200",
            "Compare 3 examples with 4 checks and 5 variables",
            "Compare 1 2 3 4 5 6 7 separate values",
            "Use labels 1 2 3 4 5 6 7 8 9 10",
            "Use labels 1.2.3.4.5.6.7.8.9.10",
            "Compare 1/2 and 3/4 fractions",
            "Student worksheet: Newton Second Law",
            "Student activity: Ohm Law",
            "Use a cell phone accelerometer to measure acceleration",
            "Contact force activity: cases 1 2 3 4 5 6 7",
            "Reference A12345",
            "Use number 123456 as a non-student example",
            "Use ISBN 9780131103627 for a source note",
            "Use EAN 4006381333931 as an example identifier",
            "Use a cell phone accelerometer and cite ISBN 9780131103627",
            "Use a cell phone accelerometer and cite ISBN 978-0-13-110362-7",
            "Use a cell phone sensor example with EAN 4006381333931",
            "Use a cell phone sensor example with EAN 400 638 1333931",
            "Use cell phone 978-0-13-110362-7 as an ISBN example",
            "Use cell phone 400 638 1333931 as an EAN example",
            "Use Unix epoch 1700000000 as a timestamp example",
            "Use identifier 12345678901 as a synthetic numeric label",
            "Compute 1+23456789 using mental math",
        ] {
            assert!(!unsafe_reasons(text).contains(&"student_pii_like_value"));
        }
    }

    #[test]
    fn student_pii_patterns_still_fail() {
        for text in [
            "teacher@example.test",
            "Grade record: Jane scored 88",
            "Student Jane Smith",
            "Student worksheet. Student Jane Smith",
            "Student name: Jane Smith",
            "Student name: Jane Q Smith",
            "Student full name: Jane Smith",
            "Learner Jane Smith",
            "Learner legal name: Jane Smith",
            "Learner legal name: Jane A Smith",
            "Roster: Jane Smith",
            "Class list entry: Jane Smith",
            "Phone 5551234567",
            "Phone 555-1234",
            "phone is 555-1234",
            "555-123-4567",
            "(555) 123-4567",
            "555 123 4567",
            "Contact: 555 123 4567",
            "Contact: 555-1234",
            "contact 555-1234",
            "call 555-1234",
            "text 555-1234",
            "555-1234 mobile",
            "5551234 phone",
            "contact me at 555-1234",
            "tel:555-1234",
            "+44 20 7946 0958",
            "+49-30-1234-5678 x123",
            "+15551234567",
            "Equation 1+2; contact +44 20 7946 0958",
            "phone 44 20 7946 0958",
            "phone 442079460958",
            "mobile 49 30 1234 5678",
            "mobile is 44 20 7946 0958",
            "mobile 4420 7946 0958",
            "call me at 44 20 7946 0958",
            "contact 44 20 7946 0958",
            "call 44 20 7946 0958",
            "text 44 20 7946 0958",
            "contact me at 44 20 7946 0958",
            "tel:44 20 7946 0958",
            "contact: 44 20 7946 0958",
            "Student ID 123-45-6789",
            "SSN 123 45 6789",
            "Student ID 123 456 789",
            "Student ID 12-345-6789",
            "student-id 123 456 789",
            "student_id 123456789",
            "Student ID A12345",
            "Learner number 123456",
            "social-security 12-345-6789",
            "Quiz score 90/100",
            "Attendance is 85%",
            "Attendance is 85 %",
            "Quiz score % 85",
        ] {
            assert!(
                unsafe_reasons(text).contains(&"student_pii_like_value"),
                "{text} should fail as student PII-like text"
            );
        }
    }

    #[test]
    fn json_object_keys_are_scanned_for_leaks() {
        let value = serde_json::json!({
            "safe": "ordinary fixture text",
            "api_key": "redacted value",
            "openai_api_key": "redacted value",
            "provider_access_token": "redacted value",
            "anthropic_client_secret": "redacted value",
            "session_token": "redacted value"
        });
        let mut findings = Vec::new();

        scan_json_strings(Path::new("fixture.json"), &value, &mut findings);

        let secret_like_count = findings
            .iter()
            .filter(|finding| finding.reason == "secret_like_value")
            .count();
        assert_eq!(
            secret_like_count, 5,
            "object keys should be scanned with the same leak rules as string values"
        );
    }

    #[test]
    fn benign_metadata_keys_do_not_fail_as_secret_like() {
        for key in ["risk_level", "source_request_summary", "sk_units"] {
            let value = serde_json::json!({ key: "ordinary fixture text" });
            let mut findings = Vec::new();

            scan_json_strings(Path::new("fixture.json"), &value, &mut findings);

            assert!(
                !findings
                    .iter()
                    .any(|finding| finding.reason == "secret_like_value"),
                "{key} should not fail as a secret-like key"
            );
        }
    }
}
