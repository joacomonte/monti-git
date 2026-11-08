// Typed wrappers over the Rust commands in src-tauri/src/lib.rs. Commands that can fail reject with git's stderr.
import { invoke } from "@tauri-apps/api/core";

export const WORKTREE_ID = "WORKTREE";

/** Size of a set of changes. The uncommitted row only counts files. */
export type Stat = { files: number; added: number; removed: number };

export type Commit = {
  id: string; // full hash, or WORKTREE_ID for the uncommitted-changes row
  parents: string[];
  author: string;
  date: number; // unix seconds
  refs: string[]; // raw %D decorations, e.g. "HEAD -> main", "origin/main", "tag: v1.0"
  subject: string; // empty on the uncommitted and saved-WIP rows: see `subject` in format.ts
  shelved?: string; // saved-WIP row: the branch whose uncommitted work Monti Git put aside when leaving it
  stat?: Stat; // uncommitted and saved-WIP rows: what they hold
};

/** [target, color, flags]: target = commit index (negative = past the loaded page); flags: 1 = the checked-out branch's line. */
export type Lane = [number, number, number] | null;

export type Row = {
  column: number;
  color: number;
  before: Lane[];
  after: Lane[];
  isCurrent: boolean;
  parents: number[]; // same ids as Lane targets
};

/** What the graph shows (history::Query in src-tauri/src/history.rs). */
export type HistoryQuery = {
  limit: number;
  onlyMine: boolean;
  branchesOnly: boolean;
  preferred: string | null; // the base the user picked for this repo
  focus: string[]; // [branch] or [branch, base]: "Only this branch"
  path: string | null; // a file's history
  environments: string[] | null; // the user's environments; null = detected by name
  only: string[] | null; // a filter's branches
};

export type History = {
  commits: Commit[];
  rows: Row[];
  names: (string | null)[]; // branch each commit belongs to, for the line tooltip
  hasMore: boolean;
  current: string | null; // checked-out branch, null when detached
  base: string | null; // the branch pinned to the left
  environments: string[]; // qa, develop…
  allBranches: string[]; // every branch name, local or on origin: the environments picker
  local: string[]; // every local branch, for "Change base"
  fastForwarded: boolean; // "Only this branch": reached the base by a fast-forward, no merge to draw
};

/** Search results, and whether some repo had more than these. */
export type Found<T> = { hits: T[]; truncated: boolean };
/** A commit whose title or description matched the global search. */
export type CommitHit = { repo: string; id: string; author: string; date: number; subject: string; snippet: string | null };
/** A line of code found by the global search, in one or more branches; `targets`: those where work ends up (qa, main…). */
export type CodeHit = { repo: string; id: string; branches: string[]; targets: string[]; path: string; line: number; text: string };
export type CodeScope = "head" | "base" | "all";
/** Tree search matches in one file: how many lines, and the first one (`old`: on the "before" side of the diff). */
export type LineHits = { count: number; line: number; old: boolean };
/** A branch containing a commit; `target`: work ends up there (an environment, main, production…). */
export type Place = { name: string; target: boolean };

/** Everything about one commit. Dates in unix seconds. */
export type CommitInfo = {
  id: string; author: string; authorEmail: string; authorDate: number; committer: string; committerEmail: string; committerDate: number;
  parents: string[]; refs: string[]; subject: string; body: string;
};

/** Who last touched each line: `lines[n - 1]` indexes `commits` for line n. An all-zero id = not committed yet. */
export type Blame = { lines: number[]; commits: { id: string; author: string; date: number; subject: string }[] };

/** A code editor the user has installed (src-tauri/src/editors.rs). */
export type Editor = { id: string; name: string };
/** How to open a repo file: selected in Finder / Explorer, with the system's default app, or in an editor. */
export type OpenWith = "reveal" | "default" | { editor: string };

/** A repo in the sidebar. `folder`: the added folder it came from. `changes`: fingerprint of its uncommitted changes (null = clean). */
export type RepoEntry = { path: string; owner: string; lastActivity: number; folder: string | null; changes: string | null };

export type Changes = {
  changes: Record<string, string>; // path -> status letter (A, M, D, R, U)
  counts: Record<string, [number, number]>; // path -> added, removed
  paths: string[]; // what the tree lists
  staged: string[]; // uncommitted-changes row only: files in the index
  untracked: string[]; // uncommitted-changes row only: new files git doesn't follow yet (deleting them can't be undone)
  nested: string[]; // the untracked ones that are a whole git repo inside this one
};

/** What deleting an untracked path loses; `nested`: it's a repo of its own (what only it has). */
export type Doomed = { path: string; nested: { unpushed: number; uncommitted: number; hasRemote: boolean } | null };

export type Side = { number: number; text: string; changed: [number, number] | null };
/** A line pair, or (with `hunk` and `header`) the "@@ … @@" separator that starts a block. */
export type DiffRow = { hunk?: number; header?: string; left: Side | null; right: Side | null; isChange: boolean };
export type FileDiff = { rows: DiffRow[]; truncated: boolean };

export type Promotion = "merged" | "fast-forwarded" | "cherry-picked" | "squashed" | "";
/** A branch card in "Commits collapsed", fetched only for the rows on screen. */
export type Card = {
  ref: string; // "qa", or "origin/qa" when there is no local branch
  local: string;
  behindBase: number;
  promotion: [string, number, Promotion][]; // target, own commits not in it yet, how they got there once 0
  firstParentCount: number; // commits "Only this branch" shows: matches the graph when focused
  hasUpstream: boolean;
  unpushed: number; // of its own commits, how many the server doesn't have yet (all of them when never pushed)
};

/** Where a branch is and whether it can go (actions::Deletion). `missing`: targets its work isn't in yet; `target`: it's
 *  the base or an environment; `fetched` false: offline, judged on the last fetch. */
export type Deletion = { local: boolean; remote: boolean; current: boolean; dirty: boolean; missing: string[]; target: boolean; fetched: boolean };

/** Dashboard: a branch with recent commits and its card facts (null without a base to compare with). */
export type ActiveBranch = { name: string; tip: string; date: number; author: string; subject: string; status: Omit<Card, "local"> | null };
/** Dashboard: one repo's recently active branches, newest first. */
export type RepoActivity = { repo: string; base: string | null; current: string | null; changed: number; branches: ActiveBranch[] };
/** A repo with the user's picks for it (null = detected). */
export type RepoPicks = { path: string; preferred: string | null; environments: string[] | null };

/** What a commit does with what's staged (actions::CommitKind). */
export type CommitKind = "new" | "amend" | { squash: string };

export type SkipReason = "diverged" | "localChanges" | "checkedOutElsewhere";
export type SyncResult = { updated: string[]; skipped: { branch: string; reason: SkipReason }[]; log: string[]; changed: boolean };

/** One git command the app ran, for the Logs panel. */
export type GitLog = {
  id: number; at: number; repo: string; args: string[]; ok: boolean; code: number | null; ms: number;
  stdout: string; stderr: string; write: boolean; truncated: boolean;
};

/** What to do with uncommitted changes when checking out: stash for this branch, WIP commit, or bring along. */
export type WipMode = "shelve" | "commit" | "carry";


export const api = {
  listRepos: (folders: string[], repos: string[]) => invoke<RepoEntry[]>("list_repos", { folders, repos }),
  isRepo: (path: string) => invoke<boolean>("is_repo", { path }),
  openFile: (repo: string, path: string, how: OpenWith) => invoke<void>("open_file", { repo, path, with: how }),
  listEditors: () => invoke<Editor[]>("list_editors"),

  loadHistory: (repo: string, query: HistoryQuery) => invoke<History>("load_history", { repo, query }),
  /** `commit` null = the uncommitted row. A commit shows what it changed against its real first parent (Span in git.rs). */
  loadChanges: (repo: string, commit: string | null, all: boolean) => invoke<Changes>("load_changes", { repo, commit, all }),
  loadDiff: (repo: string, path: string, commit: string | null, whole: boolean) => invoke<FileDiff>("load_diff", { repo, path, commit, whole }),
  commitInfo: (repo: string, id: string) => invoke<CommitInfo>("commit_info", { repo, id }),
  blame: (repo: string, path: string, rev: string | null) => invoke<Blame>("blame", { repo, path, rev }),
  branchStatus: (repo: string, refs: string[], base: string, targets: string[]) =>
    invoke<Omit<Card, "local">[]>("branch_status", { repo, refs, base, targets }),
  /** Filters, fast pass: every branch and, per target, the ones already in it by history. */
  mergedInto: (repo: string, targets: string[], onlyMine: boolean) =>
    invoke<{ all: string[]; merged: [string, string[]][] }>("merged_into", { repo, targets, onlyMine }),
  /** Dashboard: in every repo, the branches with a commit in the last `days` (yours only with `onlyMine`). */
  recentActivity: (repos: RepoPicks[], days: number, onlyMine: boolean) => invoke<RepoActivity[]>("recent_activity", { repos, days, onlyMine }),
  /** Dashboard › Safe to delete: up to `limit` from `start`; `next` is where the following page starts (null: no more). */
  safeToDelete: (repos: RepoPicks[], onlyMine: boolean, start: number, limit: number) =>
    invoke<{ items: (ActiveBranch & { repo: string })[]; next: number | null }>("safe_to_delete", { repos, onlyMine, start, limit }),
  branchTip: (repo: string, branch: string) => invoke<string | null>("branch_tip", { repo, branch }),
  lastCommitMessage: (repo: string) => invoke<string>("last_commit_message", { repo }),
  headPushed: (repo: string) => invoke<boolean>("head_pushed", { repo }),
  changedFileCount: (repo: string) => invoke<number>("changed_file_count", { repo }),
  gitLogs: (since: number) => invoke<GitLog[]>("git_logs", { since }),

  searchCommits: (repos: string[], query: string) => invoke<Found<CommitHit>>("search_commits", { repos, query }),
  searchCode: (repos: string[], bases: Record<string, string>, query: string, scope: CodeScope) =>
    invoke<Found<CodeHit>>("search_code", { repos, bases, query, scope }),
  /** Tree search by content: path -> matching lines. `paths` empty = every file. */
  searchLines: (repo: string, commit: string | null, query: string, paths: string[], diffOnly: boolean) =>
    invoke<Record<string, LineHits>>("search_lines", { repo, commit, query, paths, diffOnly }),
  commitPlaces: (repo: string, id: string) => invoke<Place[]>("commit_places", { repo, id }),

  sync: (repo: string) => invoke<SyncResult>("sync", { repo }),
  checkout: (repo: string, name: string, wip: WipMode | null) => invoke<string>("checkout", { repo, name, wip }),
  pull: (repo: string, name: string) => invoke<string>("pull", { repo, name }),
  /** Checked again in Rust right before: refused while its work is missing from a target. */
  deleteBranch: (repo: string, branch: string, base: string, targets: string[], remote: boolean) =>
    invoke<string>("delete_branch", { repo, branch, base, targets, remote }),
  /** Fetches, then where `branch` is and whether deleting it can lose work. */
  deletion: (repo: string, branch: string, base: string, targets: string[]) => invoke<Deletion>("deletion", { repo, branch, base, targets }),
  /** Fetches, then how `base` compares with origin/`base` (`remote` false: no copy there). */
  baseStatus: (repo: string, base: string) => invoke<{ ahead: number; behind: number; remote: boolean }>("base_status", { repo, base }),
  startFeature: (repo: string, name: string, base: string, wip: WipMode | null) => invoke<string>("start_feature", { repo, name, base, wip }),
  updateBranch: (repo: string, branch: string, parent: string, rebase: boolean) => invoke<void>("update_branch", { repo, branch, parent, rebase }),
  /** Sets the upstream the first time. "behind": refused, the server has commits the branch doesn't. */
  push: (repo: string, branch: string, force: boolean) => invoke<"done" | "behind">("push", { repo, branch, force }),
  moveBranch: (repo: string, branch: string, expected: string | null, target: string) => invoke<void>("move_branch", { repo, branch, expected, target }),
  discardFile: (repo: string, path: string) => invoke<void>("discard_file", { repo, path }),
  discardAll: (repo: string) => invoke<void>("discard_all", { repo }),
  untrackedCheck: (repo: string, paths: string[]) => invoke<Doomed[]>("untracked_check", { repo, paths }),
  deleteUntracked: (repo: string, paths: string[]) => invoke<void>("delete_untracked", { repo, paths }),
  /** The block the diff pane showed as `index`, with its "@@" `header` (refused if the file changed since). */
  discardHunk: (repo: string, path: string, index: number, header: string) => invoke<void>("discard_hunk", { repo, path, index, header }),
  stage: (repo: string, paths: string[]) => invoke<void>("stage", { repo, paths }),
  unstage: (repo: string, paths: string[]) => invoke<void>("unstage", { repo, paths }),
  /** A new commit, into the last one (`message` null keeps its message), or the branch squashed into one since a base. */
  commit: (repo: string, message: string | null, kind: CommitKind, push: boolean, rewrite = false) => invoke<string>("commit", { repo, message, kind, push, rewrite }),
};
