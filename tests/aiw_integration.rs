use clapback::{Command, CommandExt, Subcommand};
use googletest::prelude::*;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "fence")]
pub struct FenceCommand<C: Command> {
    #[command(current_dir)]
    #[env("TMPDIR")]
    pub workspace_path: PathBuf,

    #[arg(long)]
    pub settings: Option<PathBuf>,

    #[command(wrap, last = true)]
    pub inner: C,
}

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "direnv", arg = "exec", ensure_user_path)]
pub struct DirenvExecCommand<C: Command> {
    pub workspace_path: PathBuf,

    #[command(wrap)]
    pub inner: C,
}

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "agy", arg = "--dangerously-skip-permissions")]
pub struct AgyCommand {
    #[command(current_dir)]
    pub workspace_path: PathBuf,

    #[arg(long)]
    pub prompt: Option<String>,
}

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "direnv", ensure_user_path)]
pub struct DirenvCommand {
    #[command(current_dir)]
    pub current_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub action: DirenvAction,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum DirenvAction {
    Allow { path: PathBuf },
}

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "jj", arg = "--no-pager", ensure_user_path)]
pub struct JjCommand {
    #[command(current_dir)]
    pub current_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub subcommand: JjSubcommand,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum JjSubcommand {
    #[command(subcommand)]
    Workspace(JjWorkspaceAction),
    Status,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum JjWorkspaceAction {
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
    Forget {
        name: String,
    },
}

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "git", ensure_user_path)]
pub struct GitCommand {
    #[command(current_dir)]
    pub current_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub subcommand: GitSubcommand,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum GitSubcommand {
    RevParse {
        #[arg(long)]
        show_toplevel: bool,
    },
    #[command(subcommand)]
    Worktree(GitWorktreeAction),
    Branch {
        #[arg(short = 'D')]
        delete_force: Option<String>,
    },
    Status {
        #[arg(long)]
        porcelain: bool,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum GitWorktreeAction {
    List {
        #[arg(long)]
        porcelain: bool,
    },
    Add {
        #[arg(short = 'b')]
        branch: Option<String>,
        path: PathBuf,
    },
    Remove {
        path: PathBuf,
    },
}

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(ensure_user_path)]
pub struct HerdrCommand {
    #[command(program)]
    pub binary: PathBuf,

    #[command(subcommand)]
    pub action: HerdrAction,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum HerdrAction {
    Launch {
        #[arg(long)]
        workspace: String,
        #[arg(long, false_long = "no-focus")]
        focus: bool,
        #[arg(long)]
        command: String,
    },
    #[command(args = ["pane", "run"])]
    PaneRun { pane_id: String, command: String },
    #[command(args = ["pane", "focus"])]
    PaneFocus { pane_id: String },
    #[command(args = ["pane", "exists"])]
    PaneExists { pane_id: String },
    #[command(args = ["pane", "kill"])]
    PaneKill { pane_id: String },
    #[command(args = ["pane", "list-children"])]
    PaneListChildren { pane_id: String },
    #[command(args = ["window", "set-badge"])]
    WindowSetBadge { pane_id: String, text: String },
}

#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "wrapper")]
pub struct OptionalWrapperCommand<C: Command> {
    #[arg(long)]
    pub verbose: bool,

    #[command(wrap, last = true)]
    pub inner: Option<C>,
}

#[googletest::test]
fn fence_direnv_agy_pipeline_composes_and_renders_shell_command() {
    let workspace_path = PathBuf::from("/home/dev/repo/.workspaces/feat");
    let fence_settings = Some(PathBuf::from("/home/dev/repo/fence.jsonc"));

    let pipeline = AgyCommand {
        workspace_path: workspace_path.clone(),
        prompt: Some("Fix the failing test".to_string()),
    }
    .wrap_if(true, |inner| DirenvExecCommand {
        workspace_path: workspace_path.clone(),
        inner,
    })
    .wrap_with(|inner| FenceCommand {
        workspace_path: workspace_path.clone(),
        settings: fence_settings,
        inner,
    });

    expect_that!(pipeline.program(), eq(OsStr::new("fence")));
    expect_that!(
        pipeline.current_dir_path(),
        some(eq(Path::new("/home/dev/repo/.workspaces/feat")))
    );
    expect_that!(pipeline.ensure_user_path(), is_true());

    let mut envs = Vec::new();
    pipeline.append_envs(&mut envs);
    expect_that!(
        &envs,
        elements_are![(
            eq(OsStr::new("TMPDIR")),
            eq(OsStr::new("/home/dev/repo/.workspaces/feat"))
        )]
    );

    expect_that!(
        pipeline.to_string(),
        eq(
            "fence --settings /home/dev/repo/fence.jsonc -- direnv exec /home/dev/repo/.workspaces/feat agy --dangerously-skip-permissions --prompt 'Fix the failing test'"
        )
    );
}

#[googletest::test]
fn fence_without_direnv_skips_middle_wrapper_via_wrap_if() {
    let workspace_path = PathBuf::from("/home/dev/repo");

    let pipeline = AgyCommand {
        workspace_path: workspace_path.clone(),
        prompt: None,
    }
    .wrap_if(false, |inner| DirenvExecCommand {
        workspace_path: workspace_path.clone(),
        inner,
    })
    .wrap_with(|inner| FenceCommand {
        workspace_path: workspace_path.clone(),
        settings: None,
        inner,
    });

    expect_that!(
        pipeline.to_string(),
        eq("fence -- agy --dangerously-skip-permissions")
    );
}

#[googletest::test]
fn optional_wrapped_command_delegates_metadata_and_handles_none() {
    let inner = AgyCommand {
        workspace_path: PathBuf::from("/work/dir"),
        prompt: None,
    };

    let some_cmd = OptionalWrapperCommand {
        verbose: true,
        inner: Some(inner),
    };
    expect_that!(
        some_cmd.to_string(),
        eq("wrapper --verbose -- agy --dangerously-skip-permissions")
    );
    expect_that!(
        some_cmd.current_dir_path(),
        some(eq(Path::new("/work/dir")))
    );

    let none_cmd: OptionalWrapperCommand<AgyCommand> = OptionalWrapperCommand {
        verbose: true,
        inner: None,
    };
    expect_that!(none_cmd.to_string(), eq("wrapper --verbose"));
    expect_that!(none_cmd.current_dir_path(), none());
}

#[googletest::test]
fn direnv_jj_git_and_herdr_commands_match_aiw_models() {
    let direnv = DirenvCommand {
        current_dir: Some(PathBuf::from("/repo")),
        action: DirenvAction::Allow {
            path: PathBuf::from("/repo/.envrc"),
        },
    };
    expect_that!(direnv.to_string(), eq("direnv allow /repo/.envrc"));

    let jj_list = JjCommand {
        current_dir: Some(PathBuf::from("/repo")),
        subcommand: JjSubcommand::Workspace(JjWorkspaceAction::List {
            template: Some("name ++ \"\\n\"".to_string()),
        }),
    };
    expect_that!(
        jj_list.to_string(),
        eq("jj --no-pager workspace list -T 'name ++ \"\\n\"'")
    );

    let git_wt_add = GitCommand {
        current_dir: Some(PathBuf::from("/repo")),
        subcommand: GitSubcommand::Worktree(GitWorktreeAction::Add {
            branch: Some("feat-x".to_string()),
            path: PathBuf::from("/repo/.workspaces/feat-x"),
        }),
    };
    expect_that!(
        git_wt_add.to_string(),
        eq("git worktree add -b feat-x /repo/.workspaces/feat-x")
    );

    let git_rev_parse = GitCommand {
        current_dir: None,
        subcommand: GitSubcommand::RevParse {
            show_toplevel: true,
        },
    };
    expect_that!(
        git_rev_parse.to_string(),
        eq("git rev-parse --show-toplevel")
    );

    let herdr_launch = HerdrCommand {
        binary: PathBuf::from("herdr"),
        action: HerdrAction::Launch {
            workspace: "feat-x".to_string(),
            focus: false,
            command: "fence -- agy --dangerously-skip-permissions".to_string(),
        },
    };
    expect_that!(
        herdr_launch.to_string(),
        eq(
            "herdr launch --workspace feat-x --no-focus --command 'fence -- agy --dangerously-skip-permissions'"
        )
    );

    let herdr_badge = HerdrCommand {
        binary: PathBuf::from("/usr/local/bin/herdr"),
        action: HerdrAction::WindowSetBadge {
            pane_id: "w1:p2".to_string(),
            text: "Running tests".to_string(),
        },
    };
    expect_that!(
        herdr_badge.to_string(),
        eq("/usr/local/bin/herdr window set-badge w1:p2 'Running tests'")
    );
}
