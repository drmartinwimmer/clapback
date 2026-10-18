use crate::command::Command;
use std::ffi::OsStr;
use std::fmt;

/// Wrapper that formats a [`Command`] as a human-readable, POSIX-quoted shell command string.
pub struct CommandDisplay<'a, C: ?Sized> {
    command: &'a C,
}

impl<'a, C: ?Sized> CommandDisplay<'a, C> {
    /// Creates a new [`CommandDisplay`] wrapper for `command`.
    #[must_use]
    pub const fn new(command: &'a C) -> Self {
        Self { command }
    }
}

impl<C: Command + ?Sized> fmt::Display for CommandDisplay<'_, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", quote_os_str(self.command.program()))?;
        for arg in self.command.to_args() {
            write!(f, " {}", quote_os_str(&arg))?;
        }
        Ok(())
    }
}

impl<C: Command + ?Sized> fmt::Debug for CommandDisplay<'_, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Returns `true` if `ch` can appear unquoted in a POSIX shell token.
const fn is_safe_shell_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '-' | '_' | '.' | '/' | ':' | '=' | '+' | '@' | ',' | '%'
        )
}

/// Formats an [`OsStr`] as a POSIX shell token, leaving safe tokens unquoted and wrapping tokens
/// with spaces or special shell characters in single quotes.
#[must_use]
pub fn quote_os_str(arg: &OsStr) -> String {
    let text = arg.to_string_lossy();
    if text.is_empty() {
        return "''".to_string();
    }
    if text.chars().all(is_safe_shell_char) {
        return text.into_owned();
    }

    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('\'');
    for ch in text.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn quote_os_str_leaves_safe_tokens_unquoted_and_quotes_special_chars() {
        expect_that!(quote_os_str(OsStr::new("fence")), eq("fence"));
        expect_that!(
            quote_os_str(OsStr::new("--settings=/tmp/fence.jsonc")),
            eq("--settings=/tmp/fence.jsonc")
        );
        expect_that!(quote_os_str(OsStr::new("")), eq("''"));
        expect_that!(
            quote_os_str(OsStr::new("name ++ \"\\n\"")),
            eq("'name ++ \"\\n\"'")
        );
        expect_that!(quote_os_str(OsStr::new("it's fine")), eq("'it'\\''s fine'"));
    }
}
