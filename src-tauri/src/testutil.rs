//! Throwaway git repos for tests: real `git` in a temp folder, removed when dropped.
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Repo(PathBuf);

impl Repo {
    pub fn new(name: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("monti-git-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Repo(dir);
        repo.git(&["init", "-q", "-b", "main"]);
        for (key, value) in [("user.name", "Tester"), ("user.email", "tester@example.com"), ("commit.gpgsign", "false")] {
            repo.git(&["config", key, value]);
        }
        repo
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Runs git in the repo and returns stdout; panics with git's message if it fails.
    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git").arg("-C").arg(&self.0).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Writes `file` and commits it on the current branch.
    pub fn commit(&self, file: &str, content: &str, message: &str) {
        std::fs::write(self.0.join(file), content).unwrap();
        self.git(&["add", file]);
        self.git(&["commit", "-q", "-m", message]);
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
