use std::ffi::{OsStr, OsString};
use std::process::ExitStatus;

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
        /// Captured standard error output, if available.
        stderr: String,
    },

    /// I/O error while spawning or waiting for the command.
    #[error("I/O error during command execution: {0}")]
    Io(#[from] std::io::Error),
}

/// Trait implemented by types that represent an executable CLI command.
pub trait Command {
    /// Returns the program name or binary path to invoke.
    fn program(&self) -> &OsStr;

    /// Returns the arguments to pass to the program.
    fn args(&self) -> Vec<OsString> {
        Vec::new()
    }

    /// Constructs a [`std::process::Command`] from this command definition.
    #[must_use]
    fn to_std_command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(self.program());
        cmd.args(self.args());
        cmd
    }
}
