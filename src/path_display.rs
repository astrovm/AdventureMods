use std::path::Path;

pub fn display_path(path: &Path) -> String {
    display_path_with_home(path, dirs::home_dir().as_deref())
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
    use std::path::PathBuf;

    fn home() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from("C:\\Users\\user")
        } else {
            PathBuf::from("/home/user")
        }
    }

    #[test]
    fn home_dir_itself() {
        assert_eq!(display_path_with_home(home().as_path(), Some(home().as_path())), "~");
    }

    #[test]
    fn child_of_home() {
        let path = home().join("games");
        assert_eq!(display_path_with_home(&path, Some(home().as_path())), "~/games");
    }

    #[test]
    fn nested_child_of_home() {
        let path = home().join("games").join("sadx");
        assert_eq!(display_path_with_home(&path, Some(home().as_path())), "~/games/sadx");
    }

    #[test]
    fn outside_home() {
        let path = PathBuf::from("/usr/local/games");
        assert_eq!(display_path_with_home(&path, Some(home().as_path())), "/usr/local/games");
    }

    #[test]
    fn no_home_dir() {
        let path = home().join("games");
        assert_eq!(display_path_with_home(&path, None), path.display().to_string());
    }
    
    #[test]
    fn home_prefix_collision() {
        let home = PathBuf::from("/home/user");
        let path = PathBuf::from("/home/userother/games");
        assert_eq!(display_path_with_home(&path, Some(home.as_path())), "/home/userother/games");
    }
}