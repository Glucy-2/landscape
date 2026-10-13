//! On-disk layout of a Landscape installation.
//!
//! Default is the FHS layout below. `--config_dir` / `LANDSCAPE_CONF_PATH`
//! selects the single-directory layout rooted at that path. If neither is
//! given but a pre-FHS `~/.landscape-router` exists, it is kept so upgrades
//! never start a router with an empty configuration.
//!
//! Directories are named `landscape-router`, not `landscape`: Ubuntu's
//! `landscape-client` already owns `/etc/landscape` and `/var/lib/landscape`.
//!
//! | Purpose                                     | FHS                                  | Single dir     |
//! |---------------------------------------------|--------------------------------------|----------------|
//! | `landscape.toml`, `landscape_init.toml`     | `/etc/landscape-router`              | `<dir>`        |
//! | database, init lock, metric, geo            | `/var/lib/landscape-router`          | `<dir>`        |
//! | logs                                        | `/var/log/landscape-router`          | `<dir>/logs`   |
//! | web UI                                      | `/usr/share/landscape-router/static` | `<dir>/static` |
//! | unix sockets, API token, hostapd configs    | `/run/landscape-router`              | `<dir>`        |

use std::path::{Path, PathBuf};

use crate::{
    INIT_FILE_NAME, INIT_LOCK_FILE_NAME, LAND_CONFIG, LANDSCAPE_DB_SQLITE_NAME,
    LANDSCAPE_LOG_DIR_NAME, LANDSCAPE_WEBROOT_DIR_NAME,
};

pub const SYSTEM_CONFIG_DIR: &str = "/etc/landscape-router";
pub const SYSTEM_DATA_DIR: &str = "/var/lib/landscape-router";
pub const SYSTEM_LOG_DIR: &str = "/var/log/landscape-router";
pub const SYSTEM_WEB_ROOT: &str = "/usr/share/landscape-router/static";
pub const SYSTEM_RUN_DIR: &str = "/run/landscape-router";

#[derive(Debug, thiserror::Error)]
#[error("failed to create {}: {source}", path.display())]
pub struct CreateDirError {
    pub path: PathBuf,
    #[source]
    pub source: std::io::Error,
}

/// Pre-FHS home directory name, resolved against `$HOME`.
pub const LEGACY_HOME_DIR_NAME: &str = ".landscape-router";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathLayout {
    /// FHS directories (`/etc`, `/var/lib`, `/var/log`, `/usr/share`, `/run`).
    System,
    /// Everything under the directory given by `--config_dir`.
    ConfigDir,
    /// Everything under an existing `~/.landscape-router`.
    LegacyHome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandscapePaths {
    pub layout: PathLayout,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub web_root: PathBuf,
    pub run_dir: PathBuf,
}

impl LandscapePaths {
    pub fn system() -> Self {
        Self {
            layout: PathLayout::System,
            config_dir: PathBuf::from(SYSTEM_CONFIG_DIR),
            data_dir: PathBuf::from(SYSTEM_DATA_DIR),
            log_dir: PathBuf::from(SYSTEM_LOG_DIR),
            web_root: PathBuf::from(SYSTEM_WEB_ROOT),
            run_dir: PathBuf::from(SYSTEM_RUN_DIR),
        }
    }

    pub fn single_dir(dir: &Path) -> Self {
        Self::rooted_at(absolutize(dir), PathLayout::ConfigDir)
    }

    fn rooted_at(dir: PathBuf, layout: PathLayout) -> Self {
        Self {
            layout,
            log_dir: dir.join(LANDSCAPE_LOG_DIR_NAME),
            web_root: dir.join(LANDSCAPE_WEBROOT_DIR_NAME),
            config_dir: dir.clone(),
            data_dir: dir.clone(),
            run_dir: dir,
        }
    }

    /// Explicit `config_dir` > existing `~/.landscape-router` > FHS.
    pub fn resolve(config_dir: Option<&Path>) -> Self {
        let legacy_home = match homedir::my_home() {
            Ok(Some(home)) => Some(home.join(LEGACY_HOME_DIR_NAME)),
            Ok(None) | Err(_) => None,
        };
        Self::resolve_with(config_dir, legacy_home.as_deref())
    }

    fn resolve_with(config_dir: Option<&Path>, legacy_home: Option<&Path>) -> Self {
        if let Some(dir) = config_dir {
            return Self::single_dir(dir);
        }
        match legacy_home {
            Some(dir) if dir.is_dir() => {
                let mut paths = Self::rooted_at(dir.to_path_buf(), PathLayout::LegacyHome);
                // A packaged UI matches the installed binary; a hand-copied
                // `<legacy>/static` left over from before may not.
                if Path::new(SYSTEM_WEB_ROOT).is_dir() {
                    paths.web_root = PathBuf::from(SYSTEM_WEB_ROOT);
                }
                paths
            }
            _ => Self::system(),
        }
    }

    /// Create the directories the daemon writes into before anything else
    /// touches them. Logs and web root are left to their owners: the log
    /// path may be overridden and the web root is read-only content.
    pub fn create_dirs(&self) -> Result<(), CreateDirError> {
        for dir in [&self.config_dir, &self.data_dir, &self.run_dir] {
            std::fs::create_dir_all(dir)
                .map_err(|source| CreateDirError { path: dir.clone(), source })?;
        }
        Ok(())
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join(LAND_CONFIG)
    }

    pub fn init_file(&self) -> PathBuf {
        self.config_dir.join(INIT_FILE_NAME)
    }

    pub fn init_lock_file(&self) -> PathBuf {
        self.data_dir.join(INIT_LOCK_FILE_NAME)
    }

    pub fn db_file(&self) -> PathBuf {
        self.data_dir.join(LANDSCAPE_DB_SQLITE_NAME)
    }
}

fn absolutize(dir: &Path) -> PathBuf {
    if dir.is_absolute() {
        return dir.to_path_buf();
    }
    let current_dir = std::env::current_dir().unwrap_or_else(|error| {
        tracing::warn!(%error, "failed to determine current directory; using relative config path");
        PathBuf::from(".")
    });
    current_dir.join(dir).components().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_dir_wins_over_legacy_home() {
        let legacy = tempfile::tempdir().unwrap();
        let explicit = tempfile::tempdir().unwrap();

        let paths = LandscapePaths::resolve_with(Some(explicit.path()), Some(legacy.path()));

        assert_eq!(paths.layout, PathLayout::ConfigDir);
        assert_eq!(paths.config_dir, explicit.path());
        assert_eq!(paths.data_dir, explicit.path());
        assert_eq!(paths.run_dir, explicit.path());
        assert_eq!(paths.log_dir, explicit.path().join(LANDSCAPE_LOG_DIR_NAME));
        assert_eq!(paths.web_root, explicit.path().join(LANDSCAPE_WEBROOT_DIR_NAME));
    }

    #[test]
    fn existing_legacy_home_is_kept() {
        let legacy = tempfile::tempdir().unwrap();

        let paths = LandscapePaths::resolve_with(None, Some(legacy.path()));

        assert_eq!(paths.layout, PathLayout::LegacyHome);
        assert_eq!(paths.db_file(), legacy.path().join(LANDSCAPE_DB_SQLITE_NAME));
    }

    #[test]
    fn missing_legacy_home_falls_back_to_system_layout() {
        let parent = tempfile::tempdir().unwrap();
        let legacy = parent.path().join(LEGACY_HOME_DIR_NAME);

        let paths = LandscapePaths::resolve_with(None, Some(&legacy));

        assert_eq!(paths, LandscapePaths::system());
        assert_eq!(paths.config_file(), Path::new("/etc/landscape-router/landscape.toml"));
        assert_eq!(
            paths.init_lock_file(),
            Path::new("/var/lib/landscape-router/landscape_init.lock")
        );
    }
}
