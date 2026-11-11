//! Each branch's pull request and deploy on GitHub, through the `gh` CLI and its own login: no token is kept here.
//! Without gh, logged out, or a repo that isn't on GitHub, there's simply none. Pull requests: one `gh pr list` per
//! repo, kept for a minute. Deploys: one `gh run list`, plus one `gh run view` per run still going, kept 4 seconds.
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

/// The token gh keeps for the account named like the repo's owner (origin on github.com), asked once per owner.
/// None: no such account logged in (an organization's repo), so gh uses its active one.
fn owner_token(repo: &Path) -> Option<String> {
    static TOKENS: LazyLock<Mutex<HashMap<String, Option<String>>>> = LazyLock::new(Mutex::default);
    let url = crate::git::read_opt(&["remote", "get-url", "origin"], repo)?;
    let owner = url.trim().split("github.com").nth(1)?.trim_start_matches([':', '/']).split('/').next()?.to_string();
    if let Some(token) = TOKENS.lock().unwrap().get(&owner) {
        return token.clone();
    }
    let token = Command::new(gh()?)
        .args(["auth", "token", "-u", &owner])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    TOKENS.lock().unwrap().insert(owner, token.clone());
    token
}

/// What GitHub knows about a repo's branches: each branch's pull request, and the newest deploy runs.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pull_requests: HashMap<String, PullRequest>,
    deploys: Vec<Deploy>,
}

pub fn status(repo: &Path) -> Status {
    Status { pull_requests: pull_requests(repo), deploys: deploys(repo) }
}

/// Branch name -> its pull request: the open one, else the newest.
fn pull_requests(repo: &Path) -> HashMap<String, PullRequest> {
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

/// Runs gh in `repo` and parses its JSON output; None if gh is missing, fails, or prints something else. As the
/// repo owner's account when gh is logged in to it (a personal repo while a work account is the active one).
fn gh_json<T: serde::de::DeserializeOwned>(repo: &Path, args: &[&str]) -> Option<T> {
    let mut command = Command::new(gh()?);
    command.args(args).current_dir(repo).env("GH_PROMPT_DISABLED", "1");
    if let Some(token) = owner_token(repo) {
        command.env("GH_TOKEN", token);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
    }
    let out = command.output().ok().filter(|o| o.status.success())?;
    serde_json::from_slice(&out.stdout).ok()
}

fn list(repo: &Path) -> Option<HashMap<String, PullRequest>> {
    let fields = "number,title,url,state,isDraft,headRefName,reviewDecision,statusCheckRollup";
    Some(by_branch(gh_json(repo, &["pr", "list", "--state", "all", "--limit", "100", "--json", fields])?))
}

// MARK: Deploys

/// The newest deploy of a commit, per workflow or environment, from either source GitHub has:
/// - a workflow run whose name says so ("deploy", "release", "publish"); `done`/`total` count its jobs' steps
///   (a job with no steps listed yet counts as one), so the card's bar fills as it goes;
/// - a Deployment that an integration reports (Vercel, Netlify…: "Production", "Preview"). No steps there:
///   `total` 0, and the bar just shows it's going.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Deploy {
    name: String, // the workflow, or the environment
    sha: String,
    url: String,
    state: &'static str, // "queued", "running", "success", "failure", "cancelled"
    done: u32,
    total: u32,
    created: String,
}

/// Every deploy of the recent commits, newest first.
fn deploys(repo: &Path) -> Vec<Deploy> {
    static CACHE: LazyLock<Mutex<HashMap<String, (Instant, Vec<Deploy>)>>> = LazyLock::new(Mutex::default);
    let key = repo.to_string_lossy().to_string();
    if let Some((at, found)) = CACHE.lock().unwrap().get(&key) {
        if at.elapsed() < Duration::from_secs(4) {
            return found.clone();
        }
    }
    let mut found: Vec<Deploy> = workflow_deploys(repo).into_iter().chain(environment_deploys(repo)).collect();
    found.sort_by(|a, b| b.created.cmp(&a.created)); // ISO 8601 in UTC: sorts as text
    CACHE.lock().unwrap().insert(key, (Instant::now(), found.clone()));
    found
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    database_id: u64,
    workflow_name: String,
    head_sha: String,
    url: String,
    status: String,             // queued, in_progress, completed, waiting…
    conclusion: Option<String>, // success, failure, cancelled…
    created_at: String,
}

#[derive(Deserialize)]
struct Jobs {
    jobs: Vec<Job>,
}

#[derive(Deserialize)]
struct Job {
    status: String,
    #[serde(default)]
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    status: String,
}

fn is_deploy(workflow: &str) -> bool {
    let name = workflow.to_lowercase();
    ["deploy", "release", "publish"].iter().any(|w| name.contains(w))
}

fn run_state(run: &Run) -> &'static str {
    match (run.status.as_str(), run.conclusion.as_deref()) {
        ("completed", Some("success")) => "success",
        ("completed", Some("cancelled" | "skipped")) => "cancelled",
        ("completed", _) => "failure",
        ("queued" | "waiting" | "pending" | "requested", _) => "queued",
        _ => "running",
    }
}

/// Steps finished out of all steps, over every job of a run.
fn progress(jobs: &[Job]) -> (u32, u32) {
    jobs.iter().fold((0, 0), |(done, total), job| match job.steps.len() {
        0 => (done + u32::from(job.status == "completed"), total + 1),
        n => (done + job.steps.iter().filter(|s| s.status == "completed").count() as u32, total + n as u32),
    })
}

/// The newest run per (workflow, commit) of the deploy workflows; the steps only of those still going.
fn workflow_deploys(repo: &Path) -> Vec<Deploy> {
    let fields = "databaseId,workflowName,headSha,url,status,conclusion,createdAt";
    let runs: Vec<Run> = gh_json(repo, &["run", "list", "--limit", "40", "--json", fields]).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    runs.into_iter()
        .filter(|r| is_deploy(&r.workflow_name) && seen.insert((r.workflow_name.clone(), r.head_sha.clone()))) // newest first
        .map(|run| {
            let state = run_state(&run);
            let (done, total) = match state {
                "running" | "queued" => gh_json::<Jobs>(repo, &["run", "view", &run.database_id.to_string(), "--json", "jobs"]).map_or((0, 0), |j| progress(&j.jobs)),
                _ => (1, 1),
            };
            Deploy { name: run.workflow_name, sha: run.head_sha, url: run.url, state, done, total, created: run.created_at }
        })
        .collect()
}

#[derive(Deserialize)]
struct Deployment {
    id: u64,
    sha: String,
    environment: String,
    created_at: String,
    creator: Option<Creator>,
}

#[derive(Deserialize)]
struct Creator {
    login: String,
}

#[derive(Deserialize)]
struct DeploymentStatus {
    state: String, // queued, pending, in_progress, success, inactive, failure, error
    log_url: Option<String>,
    target_url: Option<String>,
}

fn deployment_state(state: &str) -> &'static str {
    match state {
        "success" | "inactive" => "success", // inactive: a newer deploy of the same environment replaced it
        "failure" | "error" => "failure",
        "in_progress" => "running",
        _ => "queued",
    }
}

/// The newest Deployment per (environment, commit), with its latest status. Those made by Actions are left out:
/// their workflow run already shows, with steps.
fn environment_deploys(repo: &Path) -> Vec<Deploy> {
    let listed: Vec<Deployment> = gh_json(repo, &["api", "repos/{owner}/{repo}/deployments?per_page=20"]).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    listed
        .into_iter()
        .filter(|d| d.creator.as_ref().is_none_or(|c| c.login != "github-actions[bot]") && seen.insert((d.environment.clone(), d.sha.clone())))
        .take(8)
        .filter_map(|d| {
            let path = format!("repos/{{owner}}/{{repo}}/deployments/{}/statuses?per_page=1", d.id);
            let latest = gh_json::<Vec<DeploymentStatus>>(repo, &["api", &path])?.into_iter().next();
            let state = latest.as_ref().map_or("queued", |s| deployment_state(&s.state));
            let url = latest.and_then(|s| s.log_url.or(s.target_url)).unwrap_or_default();
            Some(Deploy { name: d.environment, sha: d.sha, url, state, done: 0, total: 0, created: d.created_at })
        })
        .collect()
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
    fn deploy_progress_counts_steps() {
        let jobs: super::Jobs = serde_json::from_str(r#"{"jobs":[
          {"status":"completed","steps":[{"status":"completed"},{"status":"completed"}]},
          {"status":"in_progress","steps":[{"status":"completed"},{"status":"in_progress"},{"status":"queued"}]},
          {"status":"queued","steps":[]}]}"#).unwrap();
        assert_eq!(super::progress(&jobs.jobs), (3, 6));
        assert!(super::is_deploy("Release") && super::is_deploy("deploy-prod") && !super::is_deploy("CI"));
    }

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
