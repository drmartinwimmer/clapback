/// Converts a Rust identifier (`snake_case` or `PascalCase`) into `kebab-case`.
pub(crate) fn to_kebab_case(ident: &str) -> String {
    let trimmed = ident.strip_prefix("r#").unwrap_or(ident);
    let mut out = String::with_capacity(trimmed.len() + 4);
    let chars: Vec<char> = trimmed.chars().collect();

    for (idx, &ch) in chars.iter().enumerate() {
        if ch == '_' || ch == '-' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
            continue;
        }

        if ch.is_ascii_uppercase() {
            let prev_is_lower_or_digit = idx
                .checked_sub(1)
                .and_then(|prev_idx| chars.get(prev_idx))
                .is_some_and(|prev| prev.is_ascii_lowercase() || prev.is_ascii_digit());
            let prev_is_upper = idx
                .checked_sub(1)
                .and_then(|prev_idx| chars.get(prev_idx))
                .is_some_and(char::is_ascii_uppercase);
            let next_is_lower = chars.get(idx + 1).is_some_and(char::is_ascii_lowercase);

            if !out.is_empty()
                && !out.ends_with('-')
                && (prev_is_lower_or_digit || (prev_is_upper && next_is_lower))
            {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }

    out
}

/// Ensures a long flag string starts with `"--"`.
pub(crate) fn normalize_long_flag(raw: &str) -> String {
    if raw.starts_with("--") {
        raw.to_string()
    } else if let Some(stripped) = raw.strip_prefix('-') {
        format!("--{stripped}")
    } else {
        format!("--{raw}")
    }
}
