//! Scratch repositories for tests.
//!
//! Every repository lives under the fixtures root: `SAMPLE_GIT_FIXTURES` if
//! set, else `sample-git-fixtures` in the system temp folder. Commands run
//! with a git config file of their own (`GIT_CONFIG_GLOBAL`, no system
//! config) and `GIT_CEILING_DIRECTORIES` at the root, so they never read
//! the developer's config or hooks and never find a repository above the
//! root. The helpers refuse any path outside the root.

#![allow(dead_code)]

use sample_git::git::{Access, Git};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub fn fixtures_root() -> PathBuf {
    let root = std::env::var_os("SAMPLE_GIT_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("sample-git-fixtures"));
    fs::create_dir_all(&root).expect("create the fixtures root");
    let root = root.canonicalize().expect("the fixtures root resolves");
    // Windows' verbatim prefix (`\\?\E:\…`) confuses path comparisons
    // with what git prints.
    match root.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => root,
    }
}

/// A fresh folder for one test, with a git that only knows the fixture
/// config.
pub struct Fixture {
    pub dir: PathBuf,
    pub git: Git,
}

impl Fixture {
    pub fn new(name: &str) -> Self {
        let root = fixtures_root();
        let dir = root.join(format!(
            "{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clear an old fixture");
        }
        fs::create_dir_all(&dir).expect("create the fixture folder");
        let config = dir.join("gitconfig");
        fs::write(
            &config,
            "[user]\n\tname = Fixture Author\n\temail = fixture@example.com\n\
             [init]\n\tdefaultBranch = main\n\
             [core]\n\tautocrlf = false\n\
             [commit]\n\tgpgsign = false\n\
             [advice]\n\tdetachedHead = false\n",
        )
        .expect("write the fixture config");
        let git = Git::new()
            .with_env("GIT_CONFIG_GLOBAL", &config)
            .with_env("GIT_CONFIG_NOSYSTEM", "1")
            .with_env("GIT_CEILING_DIRECTORIES", &root);
        Fixture { dir, git }
    }

    /// `dir/name`, which must stay inside the fixture.
    pub fn path(&self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        assert!(
            path.starts_with(&self.dir) && !name.contains(".."),
            "fixture paths stay inside the fixture: {name}"
        );
        path
    }

    /// Run git in `repo` (inside the fixture) and return stdout.
    pub fn git(&self, repo: &Path, args: &[&str]) -> String {
        assert!(
            repo.starts_with(&self.dir),
            "refusing to run git outside the fixture: {}",
            repo.display()
        );
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let output = self
            .git
            .run(repo, &args, Access::Write)
            .unwrap_or_else(|error| panic!("git {args:?} in {}: {error}", repo.display()));
        String::from_utf8_lossy(&output).into_owned()
    }

    /// A new repository `name` with no commits.
    pub fn init(&self, name: &str) -> PathBuf {
        let repo = self.path(name);
        fs::create_dir_all(&repo).unwrap();
        self.git(&repo, &["init", "--quiet"]);
        repo
    }

    /// A repository with one commit containing `README.md`.
    pub fn repo_with_commit(&self, name: &str) -> PathBuf {
        let repo = self.init(name);
        write(&repo, "README.md", "# Fixture\n\nFirst line.\n");
        self.git(&repo, &["add", "README.md"]);
        self.git(&repo, &["commit", "--quiet", "-m", "Initial commit"]);
        repo
    }

    /// A bare `origin` and a clone `name` of it with one pushed commit.
    pub fn clone_with_origin(&self, name: &str) -> (PathBuf, PathBuf) {
        let origin = self.path(&format!("{name}-origin.git"));
        fs::create_dir_all(&origin).unwrap();
        self.git(&origin, &["init", "--quiet", "--bare"]);
        let seed = self.repo_with_commit(&format!("{name}-seed"));
        self.git(
            &seed,
            &["remote", "add", "origin", &origin.to_string_lossy()],
        );
        self.git(&seed, &["push", "--quiet", "-u", "origin", "main"]);
        let clone = self.path(name);
        self.git(
            &self.dir,
            &[
                "clone",
                "--quiet",
                &origin.to_string_lossy(),
                &clone.to_string_lossy(),
            ],
        );
        (origin, clone)
    }
}

pub fn write(repo: &Path, name: &str, contents: &str) {
    let path = repo.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}
