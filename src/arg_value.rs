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

    /// Emits this flag and a single value into `out`, respecting `self.require_equals`.
    pub fn push_flag_and_value(&self, out: &mut Vec<OsString>, value: OsString) {
        if self.require_equals {
            let mut combined = OsString::from(self.flag);
            combined.push("=");
            combined.push(value);
            out.push(combined);
        } else {
            out.push(OsString::from(self.flag));
            out.push(value);
        }
    }
}

/// Accumulator for joining multiple [`ArgValue`] items with a character delimiter.
#[derive(Debug, Clone)]
pub struct DelimitedBuffer {
    buf: OsString,
    delimiter: char,
    has_values: bool,
}

impl DelimitedBuffer {
    /// Creates a new empty [`DelimitedBuffer`] separated by `delimiter`.
    #[must_use]
    pub fn new(delimiter: char) -> Self {
        Self {
            buf: OsString::new(),
            delimiter,
            has_values: false,
        }
    }

    /// Appends a single raw [`OsStr`] token to the buffer, inserting the delimiter if non-empty.
    pub fn push(&mut self, val: impl AsRef<OsStr>) {
        if self.has_values {
            let mut delim_buf = [0_u8; 4];
            self.buf.push(self.delimiter.encode_utf8(&mut delim_buf));
        }
        self.has_values = true;
        self.buf.push(val.as_ref());
    }

    /// Serializes `value`'s positional tokens into this delimiter-separated buffer.
    pub fn append<V: ArgValue + ?Sized>(&mut self, value: &V) {
        let mut tokens = Vec::new();
        value.append_positional(&mut tokens);
        for token in tokens {
            self.push(&token);
        }
    }

    /// Consumes the buffer and returns the joined [`OsString`] if at least one value was pushed.
    #[must_use]
    pub fn finish(self) -> Option<OsString> {
        self.has_values.then_some(self.buf)
    }
}

/// Trait implemented by field types that can be serialized into CLI argument tokens.
pub trait ArgValue {
    /// Appends positional argument token(s) for this value to `out`.
    fn append_positional(&self, out: &mut Vec<OsString>);

    /// Appends named option or flag token(s) for this value to `out` according to `spec`.
    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>);

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
}

impl ArgValue for String {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(OsString::from(self));
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        spec.push_flag_and_value(out, OsString::from(self));
    }
}

impl ArgValue for str {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(OsString::from(self));
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        spec.push_flag_and_value(out, OsString::from(self));
    }
}

impl ArgValue for OsString {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.clone());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        spec.push_flag_and_value(out, self.clone());
    }
}

impl ArgValue for OsStr {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.to_os_string());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        spec.push_flag_and_value(out, self.to_os_string());
    }
}

impl ArgValue for PathBuf {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.as_os_str().to_os_string());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        spec.push_flag_and_value(out, self.as_os_str().to_os_string());
    }
}

impl ArgValue for Path {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        out.push(self.as_os_str().to_os_string());
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        spec.push_flag_and_value(out, self.as_os_str().to_os_string());
    }
}

impl ArgValue for Cow<'_, str> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
    }
}

impl ArgValue for Cow<'_, OsStr> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
    }
}

impl ArgValue for Cow<'_, Path> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
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
                    spec.push_flag_and_value(out, OsString::from(self.to_string()));
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
}

impl<T: ArgValue + ?Sized> ArgValue for Box<T> {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        (**self).append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        (**self).append_named(out, spec);
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
            let mut joined = DelimitedBuffer::new(delimiter);
            for item in self {
                joined.append(item);
            }
            if let Some(combined) = joined.finish() {
                spec.push_flag_and_value(out, combined);
            }
        } else {
            for item in self {
                item.append_named(out, spec);
            }
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
}

impl<T: ArgValue, const N: usize> ArgValue for [T; N] {
    fn append_positional(&self, out: &mut Vec<OsString>) {
        self.as_slice().append_positional(out);
    }

    fn append_named(&self, out: &mut Vec<OsString>, spec: &NamedArgSpec<'_>) {
        self.as_slice().append_named(out, spec);
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
