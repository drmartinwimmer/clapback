# `clapback` Implementation Plan

## 1. Vision & Goals

While [`clap`](https://docs.rs/clap) parses incoming CLI arguments into Rust structs, **`clapback`** does the inverse: users define outbound CLI invocations as strongly typed Rust structs and enums using familiar `#[command(...)]` and `#[arg(...)]` attributes, compose wrapper commands, and execute them directly via trait methods.

### Design Goals

1. **Familiar `clap`-Style Ergonomics**: Mirror `clap`'s mental model (`#[derive(Command)]`, `#[derive(Subcommand)]`, `#[derive(Args)]`, `#[derive(ValueEnum)]`, `#[command(...)]`, `#[arg(...)]`) so writing an outbound CLI wrapper feels identical to writing a `clap` parser.
2. **First-Class Command Composition**: Make wrapping one command inside another (e.g. `fence -- direnv exec . agy ...`) declarative, type-safe, and conditionally composable (`wrap`, `wrap_if`).
3. **Rich Execution & Output APIs**: Provide ergonomic methods on every command struct to build a `std::process::Command`, format a dry-run command line (`Display`), run interactively with inherited stdio, capture text/lines, or deserialize structured JSON output.
4. **Full `aiw` Compatibility**: Support every CLI invocation and composition pattern used in [`aiw`](https://github.com/drmartinwimmer/aiw) (`fence`, `direnv`, `agy`, `jj`, `git`, and `herdr`) so `aiw` can migrate all of `src/tools/*` and `src/sandbox.rs` to `clapback`.

---

## 2. Core Architecture & Traits

### 2.1 `Command` and `ToArgs` Traits (`clapback`)

```rust
/// Converts a struct, enum subcommand, or field value into CLI arguments.
pub trait ToArgs {
    /// Appends serialized CLI arguments in declaration order.
    fn append_args(&self, out: &mut Vec<OsString>);

    /// Returns the serialized CLI arguments as a `Vec<OsString>`.
    fn to_args(&self) -> Vec<OsString> {
        let mut out = Vec::new();
        self.append_args(&mut out);
        out
    }
}

/// Represents a complete, executable CLI command with a program binary, arguments,
/// working directory, and environment configuration.
pub trait Command: ToArgs {
    /// Returns the executable binary name or path (e.g. `"git"`, `"fence"`, or a custom `PathBuf`).
    fn program(&self) -> &OsStr;

    /// Optional working directory configured on the command struct (`#[command(current_dir)]`).
    fn current_dir_path(&self) -> Option<&Path> {
        None
    }

    /// Appends environment variable overrides configured on the command struct (`#[env(...)]`).
    fn append_envs(&self, _out: &mut Vec<(OsString, OsString)>) {}

    /// Returns the full argv vector (`[program, args...]`), used when composing this command
    /// as the payload of an outer wrapper command.
    fn to_argv(&self) -> Vec<OsString> {
        let mut argv = vec![self.program().to_os_string()];
        self.append_args(&mut argv);
        argv
    }
}
```

### 2.2 Value Serialization (`ArgValue`)

Field types implement `ArgValue` (or `IntoArgValues`) to control how values become `OsString` tokens:

- **Single values**: `String`, `&str`, `OsString`, `&OsStr`, `PathBuf`, `&Path`, integer/float primitives, `IpAddr`, and types implementing `ValueEnum`.
- **Optional values (`Option<T>`)**: Emits the flag/option and value when `Some(v)`, or omits both when `None`.
- **Boolean flags (`bool`)**: Emits `--flag` (or `-f`) when `true`; supports `false_long = "no-..."` when `false` must also emit an explicit flag (e.g. `herdr tab create --focus` vs `--no-focus`).
- **Repeated values (`Vec<T>`, `&[T]`)**:
  - For positional/trailing args: appends each element as a separate token.
  - For named options (`#[arg(long)]`): repeats `--flag <val>` for each item, or joins with `value_delimiter = ','`.

---

## 3. Derive Macros & Attribute Grammar (`clapback-derive`)

### 3.1 `#[derive(Command)]` (Top-Level Executable Struct)

Container attributes (`#[command(...)]`):

- `#[command(program = "git")]` or `#[command(name = "git")]`: Static binary name.
- `#[command(arg = "--no-pager")]` or `#[command(args = ["--no-pager"])]`: Static prefix arguments always passed before field/subcommand arguments (used by `GitCommand` and `JjCommand`).
- `#[command(ensure_user_path)]`: Opt-in hook to ensure user profile binary directories (`~/.local/bin`, `~/.nix-profile/bin`, `/etc/profiles/per-user/$USER/bin`) are present in `PATH`.

Field attributes:

- `#[command(program)]`: Marks a `PathBuf` / `OsString` / `String` field as a runtime override for the binary path (used by `HerdrCommand` for `HERDR_BIN_PATH`).
- `#[command(current_dir)]`: Marks a `PathBuf` or `Option<PathBuf>` field as the process working directory (`cmd.current_dir(...)`).
- `#[env("TMPDIR")]`: Binds a field (`T` or `Option<T>`) to an environment variable on the spawned process (`FenceCommand` sets `TMPDIR` to the workspace path).
- `#[command(subcommand)]`: Delegates subcommand name and arguments to a nested `#[derive(Subcommand)]` enum or `#[derive(Args)]` struct.
- `#[command(flatten)]`: Flattens another `#[derive(Args)]` struct's arguments inline.
- `#[command(wrap)]` / `#[command(wrap, last = true)]`: Appends a nested `Command`'s full `to_argv()` (`[program, args...]`), optionally preceded by `--` when `last = true`.

### 3.2 `#[derive(Subcommand)]` (Subcommand Enum)

Variant attributes (`#[command(...)]`):

- Default variant name conversion to `kebab-case` (e.g. `RevParse` -> `"rev-parse"`, `SendKeys` -> `"send-keys"`).
- `#[command(name = "...")]`: Explicit subcommand token override.
- `#[command(subcommand)]`: Nested subcommand hierarchy (e.g. `jj workspace add`, `git worktree list`, `herdr tab create`).

### 3.3 `#[derive(Args)]` (Reusable Argument Group)

Supports the same field attributes (`#[arg(...)]`, `#[command(subcommand)]`, `#[command(flatten)]`, `#[command(wrap)]`) for structs embedded inside enum variants or flattened into commands.

### 3.4 `#[arg(...)]` Field Attributes

| Attribute                               | Behavior                                                                    | Example Output                   |
| --------------------------------------- | --------------------------------------------------------------------------- | -------------------------------- |
| _(no flag attribute)_                   | Positional argument in declaration order                                    | `<value>`                        |
| `#[arg(long)]`                          | Named long option (`--field-name <value>` or `--field-name` for `bool`)     | `--settings /path/fence.jsonc`   |
| `#[arg(long = "custom")]`               | Named long option with explicit flag name                                   | `--no-pager`                     |
| `#[arg(short)]` / `#[arg(short = 'T')]` | Short option (`-T <value>` or `-f` for `bool`)                              | `-T 'name ++ "\n"'`              |
| `#[arg(long, false_long = "no-focus")]` | Boolean flag that emits `--focus` when `true` and `--no-focus` when `false` | `--focus` / `--no-focus`         |
| `#[arg(long, require_equals = true)]`   | Emits single token `--key=value`                                            | `--color=never`                  |
| `#[arg(last = true)]`                   | Emits `--` before the positional/trailing value(s) when non-empty           | `-- arg1 arg2`                   |
| `#[arg(trailing_var_arg = true)]`       | Appends variadic trailing arguments (`Vec<String>`) verbatim                | `--dangerously-skip-permissions` |

### 3.5 `#[derive(ValueEnum)]`

Converts unit enum variants into `kebab-case` (or `#[value(name = "...")]`) CLI argument strings.

---

## 4. Command Composition Design

A key requirement is effortless composition of wrapper commands, modeled directly on `aiw`'s `fence` + `direnv` + `agy` pipeline.

### 4.1 Static Struct Embedding (`#[command(wrap)]`)

Any struct field annotated with `#[command(wrap)]` (or `#[command(wrap, last = true)]`) accepts any `C: Command` (or `Option<C>`, or `Box<dyn Command>`):

```rust
#[derive(Command, Debug)]
#[command(program = "direnv")]
pub struct DirenvExecCommand<C: Command> {
    #[command(arg = "exec")]
    #[arg]
    pub dir: PathBuf,

    #[command(wrap)]
    pub command: C,
}

#[derive(Command, Debug)]
#[command(program = "fence", ensure_user_path)]
pub struct FenceCommand<C: Command> {
    #[command(current_dir)]
    #[env("TMPDIR")]
    pub workspace_path: PathBuf,

    #[arg(long)]
    pub settings: Option<PathBuf>,

    #[command(wrap, last = true)]
    pub command: C,
}
```

### 4.2 Fluent Composition & Conditional Wrapping (`CommandExt`)

To support conditional layers (such as `aiw`'s `use_direnv: bool` toggle without duplicating types), `clapback` provides `EitherCommand<L, R>` and fluent combinators on `CommandExt`:

```rust
pub enum EitherCommand<L, R> {
    Left(L),
    Right(R),
}

impl<L: Command, R: Command> Command for EitherCommand<L, R> { ... }

pub trait CommandExt: Command + Sized {
    /// Wraps `self` inside an outer command constructor `F: FnOnce(Self) -> W`.
    fn wrap_with<W: Command>(self, wrapper: impl FnOnce(Self) -> W) -> W {
        wrapper(self)
    }

    /// Conditionally wraps `self` when `condition` is true, returning an `EitherCommand<W, Self>`
    /// that itself implements `Command`.
    fn wrap_if<W: Command>(
        self,
        condition: bool,
        wrapper: impl FnOnce(Self) -> W,
    ) -> EitherCommand<W, Self> {
        if condition {
            EitherCommand::Left(wrapper(self))
        } else {
            EitherCommand::Right(self)
        }
    }
}
```

With `wrap_if` and `wrap_with`, `aiw`'s entire `SandboxBuilder` pipeline reduces to:

```rust
let sandbox_cmd = AgyCommand::new(extra_args)
    .wrap_if(direnv.is_allowed(), |cmd| DirenvExecCommand {
        dir: PathBuf::from("."),
        command: cmd,
    })
    .wrap_with(|cmd| FenceCommand {
        workspace_path: workspace.path().to_path_buf(),
        settings: resolved_settings_path,
        command: cmd,
    });
```

---

## 5. Execution & Formatting API

Every `T: Command` automatically gains execution and formatting capabilities via `CommandExt` (and `DisplayCommand`):

1. **`to_std_command(&self) -> std::process::Command`**:
   - Constructs `std::process::Command::new(self.program())`.
   - Applies `current_dir`, `append_envs`, `ensure_user_path` (if enabled), and `append_args`.
2. **`display(&self) -> CommandDisplay<'_>`** (and `impl Display` generated by `#[derive(Command)]`):
   - Formats `program` and `args` into a human-readable, POSIX-quoted command string (`fence --settings /path -- direnv exec . agy ...`), suitable for `--dry-run` output and passing to multiplexers like `herdr`.
3. **`run(&self) -> Result<(), CommandError>`**:
   - Verifies binary existence, inherits `stdin`/`stdout`/`stderr`, spawns and waits for completion, and returns `Err(CommandError::ExecutionFailed { ... })` on non-zero exit status.
4. **`output(&self) -> Result<CommandOutput, CommandError>`**:
   - Captures `stdout` and `stderr`, supports configurable `ETXTBSY` (OS error 26) retry backoff, and checks exit status.
5. **`stdout_text(&self) -> Result<String, CommandError>`**:
   - Runs `output()` and returns trimmed UTF-8 `stdout`.
6. **`output_json<D: DeserializeOwned>(&self) -> Result<D, CommandError>`** (enabled with `json` feature, default on):
   - Executes the command and deserializes `stdout` via `serde_json`.

---

## 6. How `aiw` Migrates to `clapback`

| `aiw` Module          | Current Implementation in `aiw`                                       | `clapback` Replacement                                                                                               |
| --------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `src/tools/mod.rs`    | Manual `ensure_user_profile_bin_paths(&mut cmd)`                      | Built-in `#[command(ensure_user_path)]` attribute on command structs                                                 |
| `src/tools/fence.rs`  | Manual `Command::new("fence")`, `current_dir`, `env("TMPDIR", ...)`   | `#[derive(Command)] #[command(program = "fence", ensure_user_path)] struct FenceCommand<C>`                          |
| `src/tools/direnv.rs` | Manual `DirenvCommand::build_cmd(&["status", "--json"])`, `allow`     | `#[derive(Command)] struct DirenvCommand` with `#[derive(Subcommand)] enum DirenvSubcommand { Status, Allow, Exec }` |
| `src/tools/jj.rs`     | Manual `JjCommand::build_cmd` with `--no-pager` and string slices     | `#[derive(Command)] #[command(program = "jj", arg = "--no-pager")] struct JjCommand` + `JjWorkspaceSubcommand`       |
| `src/tools/git.rs`    | Manual `GitCommand::build_cmd` with `--no-pager` and string slices    | `#[derive(Command)] #[command(program = "git", arg = "--no-pager")] struct GitCommand` + `GitSubcommand`             |
| `src/tools/herdr.rs`  | Manual `Command::new(&self.binary)` with `ETXTBSY` retry loop         | `#[derive(Command)] struct HerdrCommand` with `#[command(program)] binary: PathBuf` + `HerdrSubcommand`              |
| `src/sandbox.rs`      | Manual `SandboxBuilder` ordering `--`, `direnv exec .`, and `Display` | Declarative `AgyCommand.wrap_if(use_direnv, DirenvExecCommand::dot).wrap_with(FenceCommand::new)`                    |

---

## 7. Milestone Roadmap (Stacked PRs)

### Milestone 1: Core Traits, `ArgValue`, and Execution Engine (`feat-core-command-traits`)

- Define `ToArgs`, `Command`, `ArgValue`, `CommandOutput`, `CommandError`, and `CommandDisplay`.
- Implement `ArgValue` for standard types (`String`, `&str`, `OsString`, `PathBuf`, primitives, `Option<T>`, `Vec<T>`, `bool`).
- Implement `CommandExt` execution methods (`to_std_command`, `run`, `output`, `stdout_text`, `output_json`), user `PATH` augmentation (`ensure_user_profile_bin_paths`), and `ETXTBSY` retry handling.

### Milestone 2: `#[derive(Command)]`, `#[derive(Args)]`, and `#[arg(...)]` (`feat-derive-command-args`)

- Expand `clapback-derive` to parse struct fields and generate `ToArgs`, `Command`, and `std::fmt::Display`.
- Support positional arguments, `#[arg(long)]`, `#[arg(short)]`, `false_long`, `require_equals`, `value_delimiter`, `last = true`, `trailing_var_arg = true`, `#[command(program)]`, `#[command(current_dir)]`, `#[env("...")]`, and `#[command(flatten)]`.

### Milestone 3: `#[derive(Subcommand)]` and `#[derive(ValueEnum)]` (`feat-derive-subcommand-value-enum`)

- Implement `#[derive(Subcommand)]` on enums (unit, tuple, and struct variants) with `kebab-case` default naming, `#[command(name = "...")]` overrides, and nested `#[command(subcommand)]` chains (`jj workspace add`, `git worktree list`, `herdr tab create`).
- Implement `#[derive(ValueEnum)]` for strongly typed flag/option enums.

### Milestone 4: Command Composition & `aiw` Integration Tests (`feat-command-composition`)

- Implement `#[command(wrap)]`, `#[command(wrap, last = true)]`, `EitherCommand<L, R>`, and `CommandExt::{wrap_with, wrap_if}`.
- Add comprehensive integration tests modeling `aiw`'s `FenceCommand` + `DirenvExecCommand` + `AgyCommand`, `JjCommand`, `GitCommand`, and `HerdrCommand`.
