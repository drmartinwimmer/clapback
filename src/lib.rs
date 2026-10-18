//! Declarative CLI command builder and runner with `clap`-style derive macros and composition.

extern crate self as clapback;

mod arg_value;
mod command;
mod compose;
mod display;
mod env;

pub use arg_value::{ArgValue, DelimitedBuffer, NamedArgSpec, ValueEnum};
pub use clapback_derive::{Args, Command, Subcommand, ValueEnum};
pub use command::{Command, CommandError, CommandExt, CommandOutput, ToArgs};
pub use compose::{
    EitherCommand, append_optional_wrapped_command, append_optional_wrapped_envs,
    append_wrapped_command, optional_wrapped_current_dir_path, optional_wrapped_ensure_user_path,
};
pub use display::{CommandDisplay, quote_os_str};
pub use env::{
    AsOptionalPath, AsProgramOsStr, EnvValue, compute_augmented_path, ensure_user_profile_bin_paths,
};

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::path::{Path, PathBuf};

    #[derive(Command, Debug, Default, PartialEq, Eq)]
    #[command(program = "git")]
    struct GitCommand;

    #[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
    enum ColorWhen {
        Auto,
        Always,
        #[value(name = "never")]
        NeverEver,
    }

    #[derive(Args, Debug, Clone, PartialEq, Eq)]
    struct CommonFlags {
        #[arg(long, require_equals = true)]
        color: Option<ColorWhen>,
        #[arg(long, value_delimiter = ',')]
        features: Vec<String>,
    }

    #[derive(Command, Debug, PartialEq, Eq)]
    #[command(program = "fence", arg = "--strict", ensure_user_path)]
    struct ConfiguredFenceCommand {
        #[command(current_dir)]
        #[env("TMPDIR")]
        workspace_path: PathBuf,

        #[arg(long)]
        settings: Option<PathBuf>,

        #[arg(long, false_long = "no-focus")]
        focus: bool,

        #[arg(short = 'T')]
        template: Option<String>,

        #[command(flatten)]
        common: CommonFlags,

        #[arg(last = true, trailing_var_arg = true)]
        payload: Vec<String>,
    }

    #[derive(Command, Debug, PartialEq, Eq)]
    struct RuntimeBinaryCommand {
        #[command(program)]
        binary: PathBuf,

        #[command(args = ["pane", "run"])]
        #[arg]
        pane_id: String,

        command_text: String,
    }

    #[derive(Subcommand, Debug, PartialEq, Eq)]
    enum JjWorkspaceAction {
        Root {
            #[arg(long)]
            name: Option<String>,
        },
        List {
            #[arg(short = 'T')]
            template: Option<String>,
        },
        Add {
            path: PathBuf,
            #[arg(long)]
            name: String,
        },
        Forget(String),
    }

    #[derive(Subcommand, Debug, PartialEq, Eq)]
    enum JjSubcommand {
        #[command(subcommand)]
        Workspace(JjWorkspaceAction),
        #[command(name = "st")]
        Status,
    }

    #[derive(Command, Debug, PartialEq, Eq)]
    #[command(program = "jj", arg = "--no-pager", ensure_user_path)]
    struct JjCliCommand {
        #[command(current_dir)]
        current_dir: Option<PathBuf>,

        #[command(subcommand)]
        subcommand: JjSubcommand,
    }

    #[googletest::test]
    fn derive_command_sets_program_name() {
        let cmd = GitCommand;
        expect_that!(cmd.program(), eq(OsStr::new("git")));
        expect_that!(cmd.args().is_empty(), is_true());
        expect_that!(cmd.to_argv(), elements_are![eq(OsStr::new("git"))]);
    }

    #[googletest::test]
    fn derive_command_and_args_serialize_flags_envs_cwd_and_display() {
        let cmd = ConfiguredFenceCommand {
            workspace_path: PathBuf::from("/work/ws"),
            settings: Some(PathBuf::from("/work/fence.jsonc")),
            focus: false,
            template: Some("name ++ \"\\n\"".to_string()),
            common: CommonFlags {
                color: Some(ColorWhen::NeverEver),
                features: vec!["a".to_string(), "b".to_string()],
            },
            payload: vec![
                "agy".to_string(),
                "--dangerously-skip-permissions".to_string(),
            ],
        };

        expect_that!(cmd.program(), eq(OsStr::new("fence")));
        expect_that!(cmd.ensure_user_path(), is_true());
        expect_that!(cmd.current_dir_path(), some(eq(Path::new("/work/ws"))));

        let mut envs = Vec::new();
        cmd.append_envs(&mut envs);
        expect_that!(
            &envs,
            elements_are![(eq(OsStr::new("TMPDIR")), eq(OsStr::new("/work/ws")))]
        );

        let args = cmd.to_args();
        expect_that!(
            &args,
            elements_are![
                eq(OsStr::new("--strict")),
                eq(OsStr::new("--settings")),
                eq(OsStr::new("/work/fence.jsonc")),
                eq(OsStr::new("--no-focus")),
                eq(OsStr::new("-T")),
                eq(OsStr::new("name ++ \"\\n\"")),
                eq(OsStr::new("--color=never")),
                eq(OsStr::new("--features")),
                eq(OsStr::new("a,b")),
                eq(OsStr::new("--")),
                eq(OsStr::new("agy")),
                eq(OsStr::new("--dangerously-skip-permissions")),
            ]
        );

        expect_that!(
            cmd.to_string(),
            eq(
                "fence --strict --settings /work/fence.jsonc --no-focus -T 'name ++ \"\\n\"' --color=never --features a,b -- agy --dangerously-skip-permissions"
            )
        );
    }

    #[googletest::test]
    fn runtime_program_field_overrides_binary_and_prefixes_field_args() {
        let cmd = RuntimeBinaryCommand {
            binary: PathBuf::from("/custom/bin/herdr"),
            pane_id: "w1:p55".to_string(),
            command_text: "echo hi".to_string(),
        };

        expect_that!(cmd.program(), eq(OsStr::new("/custom/bin/herdr")));
        let argv: Vec<OsString> = cmd.to_argv();
        expect_that!(
            &argv,
            elements_are![
                eq(OsStr::new("/custom/bin/herdr")),
                eq(OsStr::new("pane")),
                eq(OsStr::new("run")),
                eq(OsStr::new("w1:p55")),
                eq(OsStr::new("echo hi")),
            ]
        );
        expect_that!(
            cmd.to_string(),
            eq("/custom/bin/herdr pane run w1:p55 'echo hi'")
        );
    }

    #[googletest::test]
    fn nested_subcommands_and_value_enum_serialize_in_declaration_order() {
        let add_cmd = JjCliCommand {
            current_dir: Some(PathBuf::from("/repo")),
            subcommand: JjSubcommand::Workspace(JjWorkspaceAction::Add {
                path: PathBuf::from("/repo/.workspaces/feat"),
                name: "feat".to_string(),
            }),
        };
        expect_that!(
            add_cmd.to_string(),
            eq("jj --no-pager workspace add /repo/.workspaces/feat --name feat")
        );

        let forget_cmd = JjCliCommand {
            current_dir: None,
            subcommand: JjSubcommand::Workspace(JjWorkspaceAction::Forget("feat".to_string())),
        };
        expect_that!(
            forget_cmd.to_string(),
            eq("jj --no-pager workspace forget feat")
        );

        let root_cmd = JjCliCommand {
            current_dir: None,
            subcommand: JjSubcommand::Workspace(JjWorkspaceAction::Root {
                name: Some("default".to_string()),
            }),
        };
        expect_that!(
            root_cmd.to_string(),
            eq("jj --no-pager workspace root --name default")
        );

        let list_cmd = JjCliCommand {
            current_dir: None,
            subcommand: JjSubcommand::Workspace(JjWorkspaceAction::List {
                template: Some("name ++ \"\\n\"".to_string()),
            }),
        };
        expect_that!(
            list_cmd.to_string(),
            eq("jj --no-pager workspace list -T 'name ++ \"\\n\"'")
        );

        let status_cmd = JjCliCommand {
            current_dir: None,
            subcommand: JjSubcommand::Status,
        };
        expect_that!(status_cmd.to_string(), eq("jj --no-pager st"));

        expect_that!(ColorWhen::Auto.as_arg_str(), eq("auto"));
        expect_that!(ColorWhen::Always.to_string(), eq("always"));
        expect_that!(ColorWhen::NeverEver.as_arg_str(), eq("never"));
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
