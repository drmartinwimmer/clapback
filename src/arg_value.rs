use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

/// Specification for serializing a named CLI option or flag (`#[arg(long)]`, `#[arg(short)]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedArgSpec<'a> {
    /// Primary flag token (e.g. `"--settings"` or `"-T"`).
    pub flag: &'a str,
    /// Optional flag emitted when a boolean field is `false` (e.g. `"--no-focus"`).
    pub false_flag: Option<&'a str>,
    /// Whether to join flag and value into a single `--flag=value` token.
    pub require_equals: bool,
    /// Optional delimiter used to join multiple values into a single token (e.g. `','`).
    pub value_delimiter: Option<char>,
}

impl<'a> NamedArgSpec<'a> {
    /// Creates a new [`NamedArgSpec`] for the given flag token.
    #[must_use]
    pub const fn new(flag: &'a str) -> Self {
        Self {
            flag,
            false_flag: None,
            require_equals: false,
            value_delimiter: None,
        }
    }

    /// Sets the flag token emitted when a boolean value is `false`.
    #[must_use]
    pub const fn with_false_flag(mut self, false_flag: &'a str) -> Self {
        self.false_flag = Some(false_flag);
        self
    }

    /// Configures whether the flag and value are joined with `=` (`--flag=value`).
    #[must_use]
    pub const fn with_require_equals(mut self, require_equals: bool) -> Self {
        self.require_equals = require_equals;
        self
    }

    /// Configures a character delimiter for joining repeated values into a single token.
    #[must_use]
    pub const fn with_value_delimiter(mut self, delimiter: char) -> Self {
        self.value_delimiter = Some(delimiter);
        self
    }
}

/// Emits a flag and single value into `out`, respecting `spec.require_equals`.
pub fn push_flag_and_value(out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>, value: OsString) {
    if spec.require_equals {
        let mut combined = OsString::from(spec.flag);
        combined.push("=");
        combined.push(value);
        out.push(combined);
    } else {
        out.push(OsString::from(spec.flag));
        out.push(value);
    }
}

/// Trait implemented by field types that can be serialized into CLI argument tokens.
pub trait ArgValue {
    /// Appends positional argument token(s) for this value to `out`.
    fn append_positional(&self, out: &mut Vec<OsString>);

    /// Appends named option or flag token(s) for this value to `out` according to `spec`.
    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>);

    /// Appends `self` to a delimiter-joined buffer for `value_delimiter`.
    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool);

    /// Appends `--` followed by positional tokens when this value produces at least one token.
    fn append_last(&self, out: &mut Vec<OsString>) {
        let mut temp = Vec::new();
        self.append_positional(&mut temp);
        if !temp.is_empty() {
            out.push(OsString::from("--"));
            out.extend(temp);
        }
    }
}

/// Helper to append a single `OsStr` token into a delimiter-separated `OsString` buffer.
pub fn push_delimited_os_str(
    buf: &mut OsString,
    val: &OsStr,
    delimiter: char,
    is_first: &mut bool,
) {
    if !*is_first {
        let mut delim_buf = [0_u8; 4];
        buf.push(delimiter.encode_utf8(&mut delim_buf));
    }
    *is_first = false;
    buf.push(val);
}

impl ArgValue for bool {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        if *self {
            out.push(OsString::from("true"));
        }
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        if *self {
            out.push(OsString::from(spec.flag));
        } else if let Some(false_flag) = spec.false_flag {
            out.push(OsString::from(false_flag));
        }
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        let s = if *self { "true" } else { "false" };
        push_delimited_os_str(buf, OsStr::new(s), delimiter, is_first);
    }
}

impl ArgValue for String {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(OsString::from(self));
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        push_flag_and_value(out, spec, OsString::from(self));
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        push_delimited_os_str(buf, OsStr::new(self), delimiter, is_first);
    }
}

impl ArgValue for str {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(OsString::from(self));
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        push_flag_and_value(out, spec, OsString::from(self));
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        push_delimited_os_str(buf, OsStr::new(self), delimiter, is_first);
    }
}

impl ArgValue for OsString {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.clone());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        push_flag_and_value(out, spec, self.clone());
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        push_delimited_os_str(buf, self.as_os_str(), delimiter, is_first);
    }
}

impl ArgValue for OsStr {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.to_os_string());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        push_flag_and_value(out, spec, self.to_os_string());
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        push_delimited_os_str(buf, self, delimiter, is_first);
    }
}

impl ArgValue for PathBuf {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.as_os_str().to_os_string());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        push_flag_and_value(out, spec, self.as_os_str().to_os_string());
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        push_delimited_os_str(buf, self.as_os_str(), delimiter, is_first);
    }
}

impl ArgValue for Path {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.as_os_str().to_os_string());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        push_flag_and_value(out, spec, self.as_os_str().to_os_string());
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        push_delimited_os_str(buf, self.as_os_str(), delimiter, is_first);
    }
}

impl ArgValue for Cow<'_, str> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        (**self).append_joined(buf, delimiter, is_first);
    }
}

impl ArgValue for Cow<'_, OsStr> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        (**self).append_joined(buf, delimiter, is_first);
    }
}

impl ArgValue for Cow<'_, Path> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        (**self).append_joined(buf, delimiter, is_first);
    }
}

macro_rules! impl_arg_value_to_string {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ArgValue for $ty {
                fn append_positional(&self, out: &mut Vec<OsString>) {
                    out.push(OsString::from(self.to_string()));
                }

                fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
                    push_flag_and_value(out, spec, OsString::from(self.to_string()));
                }

                fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
                    let s = self.to_string();
                    push_delimited_os_str(buf, OsStr::new(&s), delimiter, is_first);
                }
            }
        )*
    };
}

impl_arg_value_to_string!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char, IpAddr,
    Ipv4Addr, Ipv6Addr, SocketAddr,
);

impl<T: ArgValue + ?Sized> ArgValue for &T {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        (**self).append_joined(buf, delimiter, is_first);
    }
}

impl<T: ArgValue + ?Sized> ArgValue for Box<T> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        (**self).append_joined(buf, delimiter, is_first);
    }
}

impl<T: ArgValue> ArgValue for Option<T> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        if let Some(val) = self {
            val.append_positional(out);
        }
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        if let Some(val) = self {
            val.append_named(out, spec);
        }
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        if let Some(val) = self {
            val.append_joined(buf, delimiter, is_first);
        }
    }
}

impl<T: ArgValue> ArgValue for [T] {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        for item in self {
            item.append_positional(out);
        }
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        if self.is_empty() {
            return;
        }
        if let Some(delimiter) = spec.value_delimiter {
            let mut joined = OsString::new();
            let mut is_first = true;
            for item in self {
                item.append_joined(&mut joined, delimiter, &mut is_first);
            }
            if !is_first {
                push_flag_and_value(out, spec, joined);
            }
        } else {
            for item in self {
                item.append_named(out, spec);
            }
        }
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        for item in self {
            item.append_joined(buf, delimiter, is_first);
        }
    }
}

impl<T: ArgValue> ArgValue for Vec<T> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        self.as_slice().append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        self.as_slice().append_named(out, spec);
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        self.as_slice().append_joined(buf, delimiter, is_first);
    }
}

impl<T: ArgValue, const N: usize> ArgValue for [T; N] {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        self.as_slice().append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        self.as_slice().append_named(out, spec);
    }

    fn append_joined(&self, buf: &mut OsString, delimiter: char, is_first: &mut bool) {
        self.as_slice().append_joined(buf, delimiter, is_first);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn positional_serialization_handles_primitives_strings_and_paths() {
        let mut out = Vec::new();
        "rev-parse".append_positional(&mut out);
        String::from("--show-toplevel").append_positional(&mut out);
        PathBuf::from("/tmp/repo").append_positional(&mut out);
        42_usize.append_positional(&mut out);

        expect_that!(
            &out,
            elements_are![
                eq(OsStr::new("rev-parse")),
                eq(OsStr::new("--show-toplevel")),
                eq(OsStr::new("/tmp/repo")),
                eq(OsStr::new("42")),
            ]
        );
    }

    #[googletest::test]
    fn named_boolean_flag_handles_true_false_and_false_long() {
        let mut out = Vec::new();
        let simple_spec = NamedArgSpec::new("--force");
        true.append_named(&mut out, &simple_spec);
        false.append_named(&mut out, &simple_spec);

        let dual_spec = NamedArgSpec::new("--focus").with_false_flag("--no-focus");
        true.append_named(&mut out, &dual_spec);
        false.append_named(&mut out, &dual_spec);

        expect_that!(
            &out,
            elements_are![
                eq(OsStr::new("--force")),
                eq(OsStr::new("--focus")),
                eq(OsStr::new("--no-focus")),
            ]
        );
    }

    #[googletest::test]
    fn named_option_handles_some_none_and_require_equals() {
        let mut out = Vec::new();
        let spec = NamedArgSpec::new("--settings");
        Some(PathBuf::from("/etc/fence.jsonc")).append_named(&mut out, &spec);
        let none_path: Option<PathBuf> = None;
        none_path.append_named(&mut out, &spec);

        let eq_spec = NamedArgSpec::new("--color").with_require_equals(true);
        "never".append_named(&mut out, &eq_spec);

        expect_that!(
            &out,
            elements_are![
                eq(OsStr::new("--settings")),
                eq(OsStr::new("/etc/fence.jsonc")),
                eq(OsStr::new("--color=never")),
            ]
        );
    }

    #[googletest::test]
    fn repeated_values_support_repeated_flags_and_value_delimiter() {
        let items = vec!["alpha".to_string(), "beta".to_string()];

        let mut repeated = Vec::new();
        items.append_named(&mut repeated, &NamedArgSpec::new("--tag"));
        expect_that!(
            &repeated,
            elements_are![
                eq(OsStr::new("--tag")),
                eq(OsStr::new("alpha")),
                eq(OsStr::new("--tag")),
                eq(OsStr::new("beta")),
            ]
        );

        let mut delimited = Vec::new();
        items.append_named(
            &mut delimited,
            &NamedArgSpec::new("--features").with_value_delimiter(','),
        );
        expect_that!(
            &delimited,
            elements_are![eq(OsStr::new("--features")), eq(OsStr::new("alpha,beta"))]
        );

        let mut delimited_eq = Vec::new();
        items.append_named(
            &mut delimited_eq,
            &NamedArgSpec::new("--features")
                .with_value_delimiter(',')
                .with_require_equals(true),
        );
        expect_that!(
            &delimited_eq,
            elements_are![eq(OsStr::new("--features=alpha,beta"))]
        );
    }

    #[googletest::test]
    fn append_last_emits_double_dash_only_when_non_empty() {
        let mut out = Vec::new();
        let empty: Vec<String> = Vec::new();
        empty.append_last(&mut out);
        expect_that!(out.is_empty(), is_true());

        let trailing = vec!["agy".to_string(), "--flag".to_string()];
        trailing.append_last(&mut out);
        expect_that!(
            &out,
            elements_are![
                eq(OsStr::new("--")),
                eq(OsStr::new("agy")),
                eq(OsStr::new("--flag")),
            ]
        );
    }
}
