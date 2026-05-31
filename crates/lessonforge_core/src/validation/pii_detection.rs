pub(crate) fn has_phone_like_number(value: &str, lower: &str) -> bool {
    contains_international_phone_candidate(value)
        || contains_unformatted_us_phone_candidate(value, lower)
        || contains_formatted_phone_candidate(value)
        || contains_contextual_phone_candidate(value, lower)
}

fn contains_international_phone_candidate(value: &str) -> bool {
    value.match_indices('+').any(|(plus_index, _)| {
        if previous_char(value, plus_index).is_some_and(|previous| previous.is_ascii_digit()) {
            return false;
        }
        let candidate = &value[plus_index + 1..];
        let end = phone_candidate_end(candidate);
        end > 0 && phone_groups_are_international(&digit_group_lengths(&candidate[..end]))
    })
}

fn contains_unformatted_us_phone_candidate(value: &str, lower: &str) -> bool {
    for (start, character) in value.char_indices() {
        if !character.is_ascii_digit() {
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
        let base = &candidate[..end];
        let groups = digit_group_lengths(base);
        if matches!(groups.as_slice(), [11]) && base.starts_with('1') {
            return true;
        }
        if matches!(groups.as_slice(), [10]) && identifier_context_before_candidate(lower, start) {
            continue;
        }
        let context_before = phone_context_before_candidate(lower, start);
        let context_after = phone_context_after_candidate(lower, start + end);
        let extension_after = phone_extension_after_candidate(lower, start + end);
        if matches!(groups.as_slice(), [10])
            && (context_before || context_after || extension_after || !base.starts_with('0'))
        {
            return true;
        }
    }
    false
}

fn contains_formatted_phone_candidate(value: &str) -> bool {
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
        let base = &candidate[..end];
        let groups = digit_group_lengths(base);
        if phone_candidate_has_separator(base)
            && matches!(groups.as_slice(), [3, 3, 4] | [1, 3, 3, 4])
        {
            return true;
        }
    }
    false
}

fn contains_contextual_phone_candidate(value: &str, lower: &str) -> bool {
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
        if matches!(groups.as_slice(), [7] | [3, 4] | [10] | [11])
            || phone_groups_are_international(&groups)
        {
            return true;
        }
    }
    false
}

fn phone_groups_are_international(groups: &[usize]) -> bool {
    let digit_count = groups.iter().copied().sum::<usize>();
    (8..=15).contains(&digit_count)
        && (groups.len() == 1
            || (groups.len() == 2 && (1..=3).contains(&groups[0]))
            || groups.len() >= 3
            || matches!(groups, [4, 4, 4]))
}

fn phone_context_before_candidate(lowercase: &str, candidate_start: usize) -> bool {
    let mut removed_contact_glue = false;
    let prefix = lowercase[..candidate_start].trim_end_matches(|character: char| {
        if matches!(character, ':' | '-' | '(') {
            removed_contact_glue = true;
        }
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.' | '(')
    });
    let mut words = prefix
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let mut removed_glue = removed_contact_glue;
    while words
        .last()
        .is_some_and(|word| matches!(*word, "is" | "at"))
    {
        words.pop();
        removed_glue = true;
    }
    contact_context_suffixes()
        .iter()
        .any(|suffix| contact_suffix_matches(&words, suffix, removed_glue))
}

fn phone_context_after_candidate(lowercase: &str, candidate_end: usize) -> bool {
    let suffix = lowercase[candidate_end..].trim_start_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.' | '/' | ')')
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
        &["cell"],
        &["cell", "number"],
        &["cell", "phone"],
        &["cell", "phone", "number"],
        &["telephone"],
        &["telephone", "number"],
        &["tel"],
    ];
    contact_prefixes
        .iter()
        .any(|prefix| words.starts_with(prefix))
}

fn phone_extension_after_candidate(lowercase: &str, candidate_end: usize) -> bool {
    let suffix = lowercase[candidate_end..].trim_start_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.')
    });
    suffix.starts_with('x') || suffix.starts_with("ext") || suffix.starts_with("extension")
}

fn identifier_context_before_candidate(lowercase: &str, candidate_start: usize) -> bool {
    let prefix = lowercase[..candidate_start].trim_end_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ':' | '-' | '.')
    });
    let words = prefix
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    words.ends_with(&["isbn"]) || words.ends_with(&["isbn", "10"])
}

fn contact_context_suffixes() -> &'static [&'static [&'static str]] {
    &[
        &["phone"],
        &["phone", "number"],
        &["mobile"],
        &["mobile", "number"],
        &["cell"],
        &["cell", "number"],
        &["cell", "phone"],
        &["cell", "phone", "number"],
        &["telephone"],
        &["telephone", "number"],
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
    ]
}

fn contact_suffix_matches(words: &[&str], suffix: &[&str], removed_glue: bool) -> bool {
    if !words.ends_with(suffix) {
        return false;
    }
    if suffix == ["cell", "phone"] {
        return removed_glue;
    }
    suffix != ["phone"] || !words.ends_with(&["cell", "phone"])
}

fn phone_candidate_has_separator(candidate: &str) -> bool {
    candidate
        .chars()
        .any(|character| matches!(character, ' ' | '-' | '.' | '/' | '(' | ')'))
}

fn phone_candidate_end(candidate: &str) -> usize {
    candidate
        .char_indices()
        .find_map(|(index, character)| (!is_phone_base_character(character)).then_some(index))
        .unwrap_or(candidate.len())
}

fn is_phone_base_character(character: char) -> bool {
    character.is_ascii_digit() || matches!(character, ' ' | '-' | '.' | '/' | '(' | ')')
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
