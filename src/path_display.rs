use std::path::{Path, PathBuf};

pub fn display_path(path: &Path) -> String {
    let host = crate::steam::library::portal_host_path(path);
    display_path_with_home(host.as_deref().unwrap_or(path), user_home().as_deref())
}

/// The user's home folder. A strict Snap points `$HOME` at its own folder
/// and passes the real one in `SNAP_REAL_HOME`.
pub fn user_home() -> Option<PathBuf> {
    std::env::var_os("SNAP_REAL_HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
}

pub fn display_path_with_home(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home
        && let Ok(rel) = path.strip_prefix(home)
    {
        if rel.as_os_str().is_empty() {
            return "~".to_string();
        }
        return format!("~/{}", rel.display());
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/user")
    }

    #[test]
    fn home_dir_itself() {
        assert_eq!(
            display_path_with_home(home().as_path(), Some(home().as_path())),
            "~"
        );
    }

    #[test]
    fn child_of_home() {
        let path = home().join("games");
        assert_eq!(
            display_path_with_home(&path, Some(home().as_path())),
            "~/games"
        );
    }

    #[test]
    fn nested_child_of_home() {
        let path = home().join("games").join("sadx");
        assert_eq!(
            display_path_with_home(&path, Some(home().as_path())),
            "~/games/sadx"
        );
    }

    #[test]
    fn outside_home() {
        let path = PathBuf::from("/usr/local/games");
        assert_eq!(
            display_path_with_home(&path, Some(home().as_path())),
            "/usr/local/games"
        );
    }

    #[test]
    fn no_home_dir() {
        let path = home().join("games");
        assert_eq!(
            display_path_with_home(&path, None),
            path.display().to_string()
        );
    }

    #[test]
    fn home_prefix_collision() {
        let home = PathBuf::from("/home/user");
        let path = PathBuf::from("/home/userother/games");
        assert_eq!(
            display_path_with_home(&path, Some(home.as_path())),
            "/home/userother/games"
        );
    }

    #[test]
    fn user_home_prefers_the_real_home_inside_a_snap() {
        let _lock = crate::test_env::lock();
        unsafe { std::env::set_var("SNAP_REAL_HOME", "/home/real") };
        assert_eq!(user_home(), Some(PathBuf::from("/home/real")));
        assert_eq!(
            display_path(Path::new("/home/real/.steam/steam")),
            "~/.steam/steam"
        );
        unsafe { std::env::set_var("SNAP_REAL_HOME", "") };
        assert_eq!(user_home(), dirs::home_dir());
        unsafe { std::env::remove_var("SNAP_REAL_HOME") };
        assert_eq!(user_home(), dirs::home_dir());
    }
}
