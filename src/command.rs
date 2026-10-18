use crate::display::CommandDisplay;
use crate::env::ensure_user_profile_bin_paths;
use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command as ProcessCommand, ExitStatus, Output, Stdio};

/// Error returned when constructing or executing a CLI command.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Executable binary was not found in `PATH`.
    #[error("Executable '{0}' not found in PATH")]
    ProgramNotFound(String),

    /// Command exited with a non-zero status code.
    #[error("Command '{program}' failed with exit status: {status}")]
    ExecutionFailed {
        /// Program name or path.
        program: String,
        /// Process exit status.
        status: ExitStatus,
        /// Captured standard output, if available.
        stdout: String,
        /// Captured standard error output, if available.
        stderr: String,
    },

    /// I/O error while spawning or waiting for the command.
    #[error("I/O error during command execution: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to deserialize JSON command output.
    #[cfg(feature = "json")]
    #[error("Failed to parse command JSON output: {0}")]
    Json(#[from] serde_json::Error),
}

/// Captured output from executing a [`Command`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Process exit status.
    pub status: ExitStatus,
    /// Raw bytes captured from `stdout`.
    pub stdout: Vec<u8>,
    /// Raw bytes captured from `stderr`.
    pub stderr: Vec<u8>,
}

impl CommandOutput {
    /// Returns `stdout` decoded as lossy UTF-8.
    #[must_use]
    pub fn stdout_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    /// Returns `stderr` decoded as lossy UTF-8.
    #[must_use]
    pub fn stderr_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }

    /// Returns trimmed UTF-8 `stdout` as an owned [`String`].
    #[must_use]
    pub fn stdout_trimmed(&self) -> String {
        self.stdout_lossy().trim().to_string()
    }
}

impl From<Output> for CommandOutput {
    fn from(output: Output) -> Self {
        Self {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

/// Converts a struct, enum subcommand, or argument group into CLI argument tokens.
pub trait ToArgs {
    /// Appends serialized CLI arguments in declaration order.
    fn append_args(&self, out: &mut Vec<OsString>);

    /// Returns the serialized CLI arguments as a `Vec<OsString>`.
    #[must_use]
    fn to_args(&self) -> Vec<OsString> {
        let mut out = Vec::new();
        self.append_args(&mut out);
        out
    }
}

impl<T: ToArgs + ?Sized> ToArgs for &T {
    fn append_args(&self, out: &mut Vec<OsString>) {
        (**self).append_args(out);
    }
}

impl<T: ToArgs + ?Sized> ToArgs for Box<T> {
    fn append_args(&self, out: &mut Vec<OsString>) {
        (**self).append_args(out);
    }
}

/// Represents a complete, executable CLI command with a program binary, arguments,
/// working directory, and environment configuration.
pub trait Command: ToArgs {
    /// Returns the executable binary name or path (e.g. `"git"`, `"fence"`, or a custom `PathBuf`).
    fn program(&self) -> &OsStr;

    /// Returns the arguments to pass to the program.
    #[must_use]
    fn args(&self) -> Vec<OsString> {
        self.to_args()
    }

    /// Optional working directory configured on the command struct (`#[command(current_dir)]`).
    #[must_use]
    fn current_dir_path(&self) -> Option<&Path> {
        None
    }

    /// Appends environment variable overrides configured on the command struct (`#[env(...)]`).
    fn append_envs(&self, _out: &mut Vec<(OsString, OsString)>) {}

    /// Returns `true` if user profile binary directories (`~/.local/bin`, `~/.nix-profile/bin`,
    /// `/etc/profiles/per-user/$USER/bin`) should be ensured in `PATH`.
    #[must_use]
    fn ensure_user_path(&self) -> bool {
        false
    }

    /// Returns the full argv vector (`[program, args...]`), used when composing this command
    /// as the payload of an outer wrapper command.
    #[must_use]
    fn to_argv(&self) -> Vec<OsString> {
        let mut argv = vec![self.program().to_os_string()];
        self.append_args(&mut argv);
        argv
    }
}

impl<C: Command + ?Sized> Command for &C {
    fn program(&self) -> &OsStr {
        (**self).program()
    }

    fn current_dir_path(&self) -> Option<&Path> {
        (**self).current_dir_path()
    }

    fn append_envs(&self, out: &mut Vec<(OsString, OsString)>) {
        (**self).append_envs(out);
    }

    fn ensure_user_path(&self) -> bool {
        (**self).ensure_user_path()
    }
}

impl<C: Command + ?Sized> Command for Box<C> {
    fn program(&self) -> &OsStr {
        (**self).program()
    }

    fn current_dir_path(&self) -> Option<&Path> {
        (**self).current_dir_path()
    }

    fn append_envs(&self, out: &mut Vec<(OsString, OsString)>) {
        (**self).append_envs(out);
    }

    fn ensure_user_path(&self) -> bool {
        (**self).ensure_user_path()
    }
}

/// Extension trait providing process construction, display formatting, and execution methods
/// for all [`Command`] types.
pub trait CommandExt: Command {
    /// Constructs a [`std::process::Command`] from this command definition, applying working
    /// directory, user profile `PATH` augmentation (if enabled), environment variables, and arguments.
    #[must_use]
    fn to_std_command(&self) -> ProcessCommand {
        let mut cmd = ProcessCommand::new(self.program());
        if let Some(dir) = self.current_dir_path() {
            cmd.current_dir(dir);
        }
        if self.ensure_user_path() {
            ensure_user_profile_bin_paths(&mut cmd);
        }
        let mut envs = Vec::new();
        self.append_envs(&mut envs);
        for (key, val) in envs {
            cmd.env(key, val);
        }
        let mut args = Vec::new();
        self.append_args(&mut args);
        cmd.args(args);
        cmd
    }

    /// Returns a [`CommandDisplay`] formatter that renders `program` and `args` as a POSIX-quoted
    /// shell string.
    #[must_use]
    fn display(&self) -> CommandDisplay<'_, Self> {
        CommandDisplay::new(self)
    }

    /// Executes the command synchronously with inherited `stdin`, `stdout`, and `stderr`.
    ///
    /// Returns [`CommandError::ExecutionFailed`] if the process exits with a non-zero status.
    fn run(&self) -> Result<(), CommandError> {
        let mut cmd = self.to_std_command();
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let program_name = self.program().to_string_lossy().into_owned();
        let status = cmd
            .status()
            .map_err(|err| map_spawn_error(err, &program_name))?;
        if !status.success() {
            return Err(CommandError::ExecutionFailed {
                program: program_name,
                status,
                stdout: String::new(),
                stderr: String::new(),
            });
        }
        Ok(())
    }

    /// Executes the command and captures `stdout` and `stderr` without checking exit status.
    fn output_unchecked(&self) -> Result<CommandOutput, CommandError> {
        let mut cmd = self.to_std_command();
        let program_name = self.program().to_string_lossy().into_owned();
        let out = cmd
            .output()
            .map_err(|err| map_spawn_error(err, &program_name))?;
        Ok(CommandOutput::from(out))
    }

    /// Executes the command, captures `stdout` and `stderr`, and verifies a zero exit status.
    ///
    /// Returns [`CommandError::ExecutionFailed`] on non-zero exit status.
    fn output(&self) -> Result<CommandOutput, CommandError> {
        let out = self.output_unchecked()?;
        if !out.status.success() {
            return Err(CommandError::ExecutionFailed {
                program: self.program().to_string_lossy().into_owned(),
                status: out.status,
                stdout: out.stdout_lossy().into_owned(),
                stderr: out.stderr_lossy().into_owned(),
            });
        }
        Ok(out)
    }

    /// Executes the command via [`CommandExt::output`] and returns trimmed UTF-8 `stdout`.
    fn stdout_text(&self) -> Result<String, CommandError> {
        let out = self.output()?;
        Ok(out.stdout_trimmed())
    }

    /// Executes the command via [`CommandExt::output`] and deserializes `stdout` as JSON.
    #[cfg(feature = "json")]
    fn output_json<D: serde::de::DeserializeOwned>(&self) -> Result<D, CommandError> {
        let out = self.output()?;
        let parsed = serde_json::from_slice(&out.stdout)?;
        Ok(parsed)
    }
}

impl<C: Command + ?Sized> CommandExt for C {}

/// Maps an `std::io::Error` from process spawning into [`CommandError::ProgramNotFound`]
/// or [`CommandError::Io`].
fn map_spawn_error(err: std::io::Error, program_name: &str) -> CommandError {
    if err.kind() == ErrorKind::NotFound {
        CommandError::ProgramNotFound(program_name.to_string())
    } else {
        CommandError::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::PathBuf;

    struct CustomEchoCommand {
        program: OsString,
        args: Vec<OsString>,
        cwd: Option<PathBuf>,
        envs: Vec<(OsString, OsString)>,
    }

    impl CustomEchoCommand {
        fn sh(script: &str) -> Self {
            Self {
                program: OsString::from("sh"),
                args: vec![OsString::from("-c"), OsString::from(script)],
                cwd: None,
                envs: Vec::new(),
            }
        }
    }

    impl ToArgs for CustomEchoCommand {
        fn append_args(&self, out: &mut Vec<OsString>) {
            out.extend(self.args.iter().cloned());
        }
    }

    impl Command for CustomEchoCommand {
        fn program(&self) -> &OsStr {
            &self.program
        }

        fn current_dir_path(&self) -> Option<&Path> {
            self.cwd.as_deref()
        }

        fn append_envs(&self, out: &mut Vec<(OsString, OsString)>) {
            out.extend(self.envs.iter().cloned());
        }
    }

    #[googletest::test]
    fn to_std_command_populates_program_args_cwd_and_envs() {
        let cmd = CustomEchoCommand {
            program: OsString::from("fence"),
            args: vec![OsString::from("--settings"), OsString::from("/tmp/f.json")],
            cwd: Some(PathBuf::from("/tmp")),
            envs: vec![(OsString::from("TMPDIR"), OsString::from("/tmp"))],
        };

        let std_cmd = cmd.to_std_command();
        expect_that!(std_cmd.get_program(), eq(OsStr::new("fence")));
        expect_that!(std_cmd.get_current_dir(), some(eq(Path::new("/tmp"))));

        let collected_args: Vec<OsString> = std_cmd.get_args().map(OsStr::to_os_string).collect();
        expect_that!(
            &collected_args,
            elements_are![eq(OsStr::new("--settings")), eq(OsStr::new("/tmp/f.json"))]
        );
    }

    #[googletest::test]
    fn display_formats_posix_command_line() {
        let cmd = CustomEchoCommand::sh("echo hello world");
        expect_that!(cmd.display().to_string(), eq("sh -c 'echo hello world'"));
    }

    #[googletest::test]
    fn stdout_text_captures_and_trims_command_output() -> Result<(), Box<dyn std::error::Error>> {
        let cmd = CustomEchoCommand::sh("printf '  hello clapback\\n  '");
        let text = cmd.stdout_text()?;
        expect_that!(text, eq("hello clapback"));
        Ok(())
    }

    #[cfg(feature = "json")]
    #[googletest::test]
    fn output_json_deserializes_structured_stdout() -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Debug, serde::Deserialize, PartialEq, Eq)]
        struct SamplePayload {
            tab_id: String,
            allowed: bool,
        }

        let cmd = CustomEchoCommand::sh(r#"printf '{"tab_id":"w1:t8","allowed":true}'"#);
        let parsed: SamplePayload = cmd.output_json()?;
        expect_that!(
            parsed,
            eq(&SamplePayload {
                tab_id: "w1:t8".to_string(),
                allowed: true,
            })
        );
        Ok(())
    }

    #[googletest::test]
    fn non_zero_exit_returns_execution_failed_error() {
        let cmd = CustomEchoCommand::sh("echo 'bad state' >&2; exit 7");
        let res = cmd.output();
        expect_that!(
            res,
            err(matches_pattern!(CommandError::ExecutionFailed {
                program: eq("sh"),
                stderr: contains_substring("bad state"),
                ..
            }))
        );
    }

    #[googletest::test]
    fn missing_binary_returns_program_not_found_error() {
        let cmd = CustomEchoCommand {
            program: OsString::from("/nonexistent/clapback/binary"),
            args: Vec::new(),
            cwd: None,
            envs: Vec::new(),
        };
        let res = cmd.output();
        expect_that!(
            res,
            err(matches_pattern!(CommandError::ProgramNotFound(eq(
                "/nonexistent/clapback/binary"
            ))))
        );
    }
}
