use crate::command::{Command, CommandExt, ToArgs};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::Path;

/// A sum type representing one of two possible [`Command`] implementations.
///
/// Returned by [`CommandExt::wrap_if`] to enable type-safe conditional command wrapping
/// without boxing or dynamic dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EitherCommand<L, R> {
    /// The left command branch (typically the wrapped command when `wrap_if` condition is `true`).
    Left(L),
    /// The right command branch (typically the unwrapped command when `wrap_if` condition is `false`).
    Right(R),
}

impl<L: ToArgs, R: ToArgs> ToArgs for EitherCommand<L, R> {
    fn append_args(&self, out: &mut Vec<OsString>) {
        match self {
            Self::Left(left) => left.append_args(out),
            Self::Right(right) => right.append_args(out),
        }
    }
}

impl<L: Command, R: Command> Command for EitherCommand<L, R> {
    fn program(&self) -> &OsStr {
        match self {
            Self::Left(left) => left.program(),
            Self::Right(right) => right.program(),
        }
    }

    fn current_dir_path(&self) -> Option<&Path> {
        match self {
            Self::Left(left) => left.current_dir_path(),
            Self::Right(right) => right.current_dir_path(),
        }
    }

    fn append_envs(&self, out: &mut Vec<(OsString, OsString)>) {
        match self {
            Self::Left(left) => left.append_envs(out),
            Self::Right(right) => right.append_envs(out),
        }
    }

    fn ensure_user_path(&self) -> bool {
        match self {
            Self::Left(left) => left.ensure_user_path(),
            Self::Right(right) => right.ensure_user_path(),
        }
    }
}

impl<L: Command, R: Command> fmt::Display for EitherCommand<L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.display(), f)
    }
}

/// Appends a wrapped [`Command`]'s full argv (`[program, args...]`) to `out`,
/// preceded by `"--"` when `last` is `true`.
pub fn append_wrapped_command<C: Command + ?Sized>(cmd: &C, out: &mut Vec<OsString>, last: bool) {
    let argv = cmd.to_argv();
    if !argv.is_empty() {
        if last {
            out.push(OsString::from("--"));
        }
        out.extend(argv);
    }
}

/// Appends an optional wrapped [`Command`]'s full argv (`[program, args...]`) to `out` when `Some`.
pub fn append_optional_wrapped_command<C: Command>(
    cmd: &Option<C>,
    out: &mut Vec<OsString>,
    last: bool,
) {
    if let Some(inner) = cmd {
        append_wrapped_command(inner, out, last);
    }
}

/// Returns the working directory of an optional wrapped [`Command`] when `Some`.
pub fn optional_wrapped_current_dir_path<C: Command>(cmd: &Option<C>) -> Option<&Path> {
    cmd.as_ref().and_then(Command::current_dir_path)
}

/// Appends environment variables from an optional wrapped [`Command`] to `out` when `Some`.
pub fn append_optional_wrapped_envs<C: Command>(
    cmd: &Option<C>,
    out: &mut Vec<(OsString, OsString)>,
) {
    if let Some(inner) = cmd {
        inner.append_envs(out);
    }
}

/// Returns `true` if an optional wrapped [`Command`] is `Some` and requests user profile `PATH` augmentation.
pub fn optional_wrapped_ensure_user_path<C: Command>(cmd: &Option<C>) -> bool {
    cmd.as_ref().is_some_and(Command::ensure_user_path)
}
