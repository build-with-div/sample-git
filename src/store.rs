//! The saved repository list: a text file, one path per line.
//!
//! It lives in `%APPDATA%\div-sample-git\repos.txt` on Windows and
//! `$XDG_CONFIG_HOME/div-sample-git/repos.txt` (or `~/.config/…`)
//! elsewhere. `SAMPLE_GIT_STORE` names another file, for development runs
//! against fixture repositories.
//!
//! div-gap: 20. Div has no app data directory API, so the platform
//! variables are read here.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const FOLDER: &str = "div-sample-git";
pub const FILE: &str = "repos.txt";

/// The file the app reads and writes, if one can be found.
pub fn default_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SAMPLE_GIT_STORE") {
        return Some(PathBuf::from(path));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
    };
    base.map(|base| base.join(FOLDER).join(FILE))
}

/// The saved paths, in order, without blanks or duplicates. A missing file
/// is an empty list.
pub fn load(path: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let path = PathBuf::from(line);
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

/// Write `repos` to `path`, creating its folder.
pub fn save(path: &Path, repos: &[PathBuf]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for repo in repos {
        text.push_str(&repo.to_string_lossy());
        text.push('\n');
    }
    // Write a sibling file and rename it over the old one, so a crash
    // never leaves half a list.
    let temporary = path.with_extension("txt.tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_round_trips_without_duplicates() {
        let dir = std::env::temp_dir().join(format!("sample-git-store-{}", std::process::id()));
        let file = dir.join("nested").join(FILE);
        assert_eq!(load(&file), Vec::<PathBuf>::new());
        let repos = vec![PathBuf::from("/a/one"), PathBuf::from("/b/two with space")];
        save(&file, &repos).unwrap();
        fs::write(
            &file,
            format!("{}\n\n/a/one\n", fs::read_to_string(&file).unwrap()),
        )
        .unwrap();
        assert_eq!(load(&file), repos);
        fs::remove_dir_all(&dir).unwrap();
    }
}
