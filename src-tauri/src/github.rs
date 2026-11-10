//! Each branch's pull request on GitHub, through the `gh` CLI and its own login: no token is kept here. Without gh,
//! logged out, or a repo that isn't on GitHub, there's simply none. One `gh pr list` per repo, kept for a minute.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    number: u32,
    title: String,
    url: String,
    state: &'static str,          // "open", "draft", "merged", "closed"
    review: Option<&'static str>, // "approved", "changes", "required"
    checks: Option<&'static str>, // "pass", "fail", "pending"; None without checks
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Listed {
    number: u32,
    title: String,
    url: String,
    state: String, // OPEN, MERGED, CLOSED
    is_draft: bool,
    head_ref_name: String,
    review_decision: Option<String>, // APPROVED, CHANGES_REQUESTED, REVIEW_REQUIRED
    #[serde(default)]
    status_check_rollup: Vec<Check>,
}

/// A check run (`status` + `conclusion`) or a commit status (`state`).
#[derive(Deserialize)]
struct Check {
    status: Option<String>,
    conclusion: Option<String>,
    state: Option<String>,
}

/// One for every check: failed if any failed, pending if any still runs, else passed.
fn checks(all: &[Check]) -> Option<&'static str> {
    let outcome = |c: &Check| c.conclusion.as_deref().filter(|s| !s.is_empty()).or(c.state.as_deref()).unwrap_or("").to_uppercase();
    let failed = |o: &str| matches!(o, "FAILURE" | "ERROR" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE");
    let running = |c: &Check| c.status.as_deref().is_some_and(|s| s != "COMPLETED") || matches!(outcome(c).as_str(), "PENDING" | "EXPECTED" | "");
    match () {
        _ if all.is_empty() => None,
        _ if all.iter().any(|c| failed(&outcome(c))) => Some("fail"),
        _ if all.iter().any(running) => Some("pending"),
        _ => Some("pass"),
    }
}

/// gh as installed by Homebrew or the installers; a GUI app on macOS doesn't get the shell's PATH.
fn gh() -> Option<&'static str> {
    static FOUND: LazyLock<Option<&'static str>> = LazyLock::new(|| {
        ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/usr/bin/gh"].into_iter().find(|p| Path::new(p).exists()).or(Some("gh"))
    });
    *FOUND
}

/// Branch name -> its pull request: the open one, else the newest.
pub fn pull_requests(repo: &Path) -> HashMap<String, PullRequest> {
    static CACHE: LazyLock<Mutex<HashMap<String, (Instant, HashMap<String, PullRequest>)>>> = LazyLock::new(Mutex::default);
    let key = repo.to_string_lossy().to_string();
    if let Some((at, found)) = CACHE.lock().unwrap().get(&key) {
        if at.elapsed() < Duration::from_secs(60) {
            return found.clone();
        }
    }
    let found = list(repo).unwrap_or_default();
    CACHE.lock().unwrap().insert(key, (Instant::now(), found.clone()));
    found
}

fn list(repo: &Path) -> Option<HashMap<String, PullRequest>> {
    let fields = "number,title,url,state,isDraft,headRefName,reviewDecision,statusCheckRollup";
    let mut command = Command::new(gh()?);
    command.args(["pr", "list", "--state", "all", "--limit", "100", "--json", fields]).current_dir(repo).env("GH_PROMPT_DISABLED", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
    }
    let out = command.output().ok().filter(|o| o.status.success())?;
    Some(by_branch(serde_json::from_slice(&out.stdout).ok()?))
}

/// Newest first, as gh lists them: a branch keeps its open pull request, else its first (newest) one.
fn by_branch(listed: Vec<Listed>) -> HashMap<String, PullRequest> {
    let mut found: HashMap<String, PullRequest> = HashMap::new();
    for pr in listed {
        let state = match (pr.state.as_str(), pr.is_draft) {
            ("MERGED", _) => "merged",
            ("CLOSED", _) => "closed",
            (_, true) => "draft",
            _ => "open",
        };
        if found.get(&pr.head_ref_name).is_some_and(|kept| kept.state == "open" || kept.state == "draft" || state == "closed" || state == "merged") {
            continue;
        }
        let review = match pr.review_decision.as_deref() {
            Some("APPROVED") => Some("approved"),
            Some("CHANGES_REQUESTED") => Some("changes"),
            Some("REVIEW_REQUIRED") => Some("required"),
            _ => None,
        };
        let checks = checks(&pr.status_check_rollup);
        found.insert(pr.head_ref_name, PullRequest { number: pr.number, title: pr.title, url: pr.url, state, review, checks });
    }
    found
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_the_open_pull_request_and_sums_up_checks() {
        let json = r#"[
          {"number":3,"title":"again","url":"u3","state":"OPEN","isDraft":false,"headRefName":"feat","reviewDecision":"APPROVED",
           "statusCheckRollup":[{"status":"COMPLETED","conclusion":"SUCCESS"},{"status":"IN_PROGRESS","conclusion":""}]},
          {"number":2,"title":"first","url":"u2","state":"CLOSED","isDraft":false,"headRefName":"feat","reviewDecision":null,"statusCheckRollup":[]},
          {"number":1,"title":"old","url":"u1","state":"MERGED","isDraft":false,"headRefName":"done","reviewDecision":"","statusCheckRollup":[{"state":"FAILURE"}]}
        ]"#;
        let found = super::by_branch(serde_json::from_str(json).unwrap());
        let feat = &found["feat"];
        assert_eq!((feat.number, feat.state, feat.review, feat.checks), (3, "open", Some("approved"), Some("pending")));
        assert_eq!((found["done"].state, found["done"].checks), ("merged", Some("fail")));
    }
}
