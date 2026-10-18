use std::env;
use std::ffi::{OsStr, OsString};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

/// Converts a field value into an optional environment variable value (`OsString`).
pub trait EnvValue {
    /// Returns `Some(OsString)` when the environment variable should be set, or `None` to omit it.
    fn to_env_os_string(&self) -> Option<OsString>;
}

impl EnvValue for String {
    fn to_env_os_string(&self) -> Option<OsString> {
        Some(OsString::from(self))
    }
}

impl EnvValue for str {
    fn to_env_os_string(&self) -> Option<OsString> {
        Some(OsString::from(self))
    }
}

impl EnvValue for OsString {
    fn to_env_os_string(&self) -> Option<OsString> {
        Some(self.clone())
    }
}

impl EnvValue for OsStr {
    fn to_env_os_string(&self) -> Option<OsString> {
        Some(self.to_os_string())
    }
}

impl EnvValue for PathBuf {
    fn to_env_os_string(&self) -> Option<OsString> {
        Some(self.as_os_str().to_os_string())
    }
}

impl EnvValue for Path {
    fn to_env_os_string(&self) -> Option<OsString> {
        Some(self.as_os_str().to_os_string())
    }
}

impl EnvValue for bool {
    fn to_env_os_string(&self) -> Option<OsString> {
        Some(OsString::from(if *self { "true" } else { "false" }))
    }
}

macro_rules! impl_env_value_to_string {
    ($($ty:ty),* $(,)?) => {
        $(
            impl EnvValue for $ty {
                fn to_env_os_string(&self) -> Option<OsString> {
                    Some(OsString::from(self.to_string()))
                }
            }
        )*
    };
}

impl_env_value_to_string!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char, IpAddr,
    Ipv4Addr, Ipv6Addr, SocketAddr,
);

impl<T: EnvValue + ?Sized> EnvValue for &T {
    fn to_env_os_string(&self) -> Option<OsString> {
        (**self).to_env_os_string()
    }
}

impl<T: EnvValue + ?Sized> EnvValue for Box<T> {
    fn to_env_os_string(&self) -> Option<OsString> {
        (**self).to_env_os_string()
    }
}

impl<T: EnvValue> EnvValue for Option<T> {
    fn to_env_os_string(&self) -> Option<OsString> {
        self.as_ref().and_then(EnvValue::to_env_os_string)
    }
}

/// Converts a working directory field (`PathBuf`, `Option<PathBuf>`, `&Path`, etc.) into `Option<&Path>`.
pub trait AsOptionalPath {
    /// Returns the working directory path if present.
    fn as_optional_path(&self) -> Option<&Path>;
}

impl AsOptionalPath for PathBuf {
    fn as_optional_path(&self) -> Option<&Path> {
        Some(self.as_path())
    }
}

impl AsOptionalPath for Path {
    fn as_optional_path(&self) -> Option<&Path> {
        Some(self)
    }
}

impl AsOptionalPath for String {
    fn as_optional_path(&self) -> Option<&Path> {
        Some(Path::new(self.as_str()))
    }
}

impl AsOptionalPath for str {
    fn as_optional_path(&self) -> Option<&Path> {
        Some(Path::new(self))
    }
}

impl AsOptionalPath for OsString {
    fn as_optional_path(&self) -> Option<&Path> {
        Some(Path::new(self.as_os_str()))
    }
}

impl AsOptionalPath for OsStr {
    fn as_optional_path(&self) -> Option<&Path> {
        Some(Path::new(self))
    }
}

impl<T: AsOptionalPath + ?Sized> AsOptionalPath for &T {
    fn as_optional_path(&self) -> Option<&Path> {
        (**self).as_optional_path()
    }
}

impl<T: AsOptionalPath> AsOptionalPath for Option<T> {
    fn as_optional_path(&self) -> Option<&Path> {
        self.as_ref().and_then(AsOptionalPath::as_optional_path)
    }
}

/// Mutable list of `PATH` entries that tracks whether any entries were added.
#[derive(Debug)]
struct PathList {
    entries: Vec<PathBuf>,
    updated: bool,
}

impl PathList {
    fn from_os_str(path_var: &OsStr) -> Self {
        Self {
            entries: env::split_paths(path_var).collect(),
            updated: false,
        }
    }

    fn prepend_if_exists(&mut self, dir: PathBuf) {
        if dir.exists() && !self.entries.contains(&dir) {
            self.entries.insert(0, dir);
            self.updated = true;
        }
    }

    fn append_if_exists(&mut self, dir: PathBuf) {
        if dir.exists() && !self.entries.contains(&dir) {
            self.entries.push(dir);
            self.updated = true;
        }
    }

    fn into_joined(self) -> Option<OsString> {
        self.updated
            .then(|| env::join_paths(self.entries).ok())
            .flatten()
    }
}

/// Ensures common user binary directories (`~/.local/bin`, `~/.nix-profile/bin`,
/// `/etc/profiles/per-user/$USER/bin`) are included in `cmd`'s `PATH`.
pub fn ensure_user_profile_bin_paths(cmd: &mut ProcessCommand) {
    let path_var = env::var_os("PATH");
    let home = env::var_os("HOME").map(PathBuf::from);
    let user = env::var("USER").ok();

    if let Some(new_path) =
        compute_augmented_path(path_var.as_deref(), home.as_deref(), user.as_deref())
    {
        cmd.env("PATH", new_path);
    }
}

/// Computes an updated `PATH` value if any existing user profile bin directories are missing.
#[must_use]
pub fn compute_augmented_path(
    current_path: Option<&OsStr>,
    home: Option<&Path>,
    user: Option<&str>,
) -> Option<OsString> {
    let mut path_list = PathList::from_os_str(current_path?);

    if let Some(home_dir) = home {
        path_list.prepend_if_exists(home_dir.join(".local/bin"));
        path_list.append_if_exists(home_dir.join(".nix-profile/bin"));
    }

    if let Some(username) = user {
        path_list.append_if_exists(PathBuf::from(format!(
            "/etc/profiles/per-user/{username}/bin"
        )));
    }

    path_list.into_joined()
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;
    use tempfile::tempdir;

    #[googletest::test]
    fn env_value_converts_standard_and_optional_types() {
        expect_that!("hello".to_env_os_string(), some(eq(OsStr::new("hello"))));
        expect_that!(
            String::from("world").to_env_os_string(),
            some(eq(OsStr::new("world")))
        );
        expect_that!(
            PathBuf::from("/tmp/ws").to_env_os_string(),
            some(eq(OsStr::new("/tmp/ws")))
        );
        expect_that!(42_u32.to_env_os_string(), some(eq(OsStr::new("42"))));
        expect_that!(true.to_env_os_string(), some(eq(OsStr::new("true"))));
        expect_that!(Some("set").to_env_os_string(), some(eq(OsStr::new("set"))));
        let none_val: Option<String> = None;
        expect_that!(none_val.to_env_os_string(), none());
    }

    #[googletest::test]
    fn as_optional_path_resolves_paths_and_options() {
        let p = PathBuf::from("/work/dir");
        expect_that!(p.as_optional_path(), some(eq(Path::new("/work/dir"))));
        let some_p = Some(PathBuf::from("/work/opt"));
        expect_that!(some_p.as_optional_path(), some(eq(Path::new("/work/opt"))));
        let none_p: Option<PathBuf> = None;
        expect_that!(none_p.as_optional_path(), none());
    }

    #[googletest::test]
    fn compute_augmented_path_prepends_local_bin_and_appends_nix_profile()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_home = tempdir()?;
        let local_bin = temp_home.path().join(".local/bin");
        let nix_bin = temp_home.path().join(".nix-profile/bin");
        fs::create_dir_all(&local_bin)?;
        fs::create_dir_all(&nix_bin)?;

        let base_path = OsStr::new("/usr/bin:/bin");
        let augmented = compute_augmented_path(Some(base_path), Some(temp_home.path()), None)
            .ok_or("expected augmented path")?;
        let split: Vec<PathBuf> = env::split_paths(&augmented).collect();

        expect_that!(split.first(), some(eq(&local_bin)));
        expect_that!(split.last(), some(eq(&nix_bin)));
        Ok(())
    }

    #[googletest::test]
    fn compute_augmented_path_returns_none_when_already_present()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_home = tempdir()?;
        let local_bin = temp_home.path().join(".local/bin");
        fs::create_dir_all(&local_bin)?;

        let existing = env::join_paths([local_bin.as_path(), Path::new("/usr/bin")])?;
        let augmented = compute_augmented_path(Some(&existing), Some(temp_home.path()), None);
        expect_that!(augmented, none());
        Ok(())
    }
}
