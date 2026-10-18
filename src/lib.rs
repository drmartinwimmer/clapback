//! Declarative CLI command builder and runner with `clap`-style derive macros and composition.

extern crate self as clapback;

mod arg_value;
mod command;
mod display;
mod env;

pub use arg_value::{ArgValue, NamedArgSpec, push_delimited_os_str, push_flag_and_value};
pub use clapback_derive::Command;
pub use command::{Command, CommandError, CommandExt, CommandOutput, ToArgs};
pub use display::{CommandDisplay, quote_os_str};
pub use env::{AsOptionalPath, EnvValue, compute_augmented_path, ensure_user_profile_bin_paths};

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::ffi::OsStr;
    use std::fs;
    use std::path::Path;

    #[derive(Command, Debug, Default, PartialEq, Eq)]
    #[command(program = "git")]
    struct GitCommand;

    #[googletest::test]
    fn derive_command_sets_program_name() {
        let cmd = GitCommand;
        expect_that!(cmd.program(), eq(OsStr::new("git")));
        expect_that!(cmd.args().is_empty(), is_true());
        expect_that!(cmd.to_argv(), elements_are![eq(OsStr::new("git"))]);
    }

    #[googletest::test]
    fn verify_msrv_badge_consistency_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml_content = fs::read_to_string(manifest_dir.join("Cargo.toml"))?;
        let rust_version_line = cargo_toml_content
            .lines()
            .find(|line| line.trim().starts_with("rust-version"))
            .ok_or("rust-version not found in Cargo.toml")?;
        let rust_version = rust_version_line
            .split('=')
            .nth(1)
            .ok_or("Invalid rust-version entry")?
            .trim()
            .trim_matches('"');

        let mut parts = rust_version.split('.');
        let major = parts.next().ok_or("Missing major version component")?;
        let minor = parts.next().ok_or("Missing minor version component")?;
        let badge_target = format!("https://img.shields.io/badge/MSRV-{major}.{minor}%2B-");

        let readme_content = fs::read_to_string(manifest_dir.join("README.md"))?;
        assert_that!(readme_content.contains(&badge_target), is_true());

        Ok(())
    }
}
