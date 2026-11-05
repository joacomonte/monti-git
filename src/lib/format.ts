// How times, counts and names read in the UI.
import { WORKTREE_ID, type Commit, type GitLog } from "./api";

/** "5m ago", "3d ago", "2mo ago", "1y ago". `now` in ms, for views that tick. */
export function ago(seconds: number, now = Date.now()) {
  const s = Math.max(0, now / 1000 - seconds);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  const days = Math.floor(s / 86400);
  return days < 30 ? `${days}d ago` : days < 365 ? `${Math.floor(days / 30)}mo ago` : `${Math.floor(days / 365)}y ago`;
}

const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

/** "5 hours ago" for the last week, a plain date after that: the graph's dates. */
export function when(seconds: number) {
  const diff = seconds - Date.now() / 1000;
  if (diff > -7 * 86400) {
    for (const [unit, size] of [["day", 86400], ["hour", 3600], ["minute", 60]] as const) {
      if (-diff >= size) return relative.format(Math.round(diff / size), unit);
    }
    return relative.format(0, "second");
  }
  const date = new Date(seconds * 1000);
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return date.toLocaleDateString(undefined, { day: "numeric", month: "short", year: sameYear ? undefined : "numeric" });
}

/** "1 file", "3 files". */
export const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** Last part of a path: the repo or file name. */
export const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** A row's subject: the commit's, or what the uncommitted and saved-WIP rows hold. */
export function subject(commit: Commit) {
  if (commit.id === WORKTREE_ID) return plural(commit.stat?.files ?? 0, "changed file");
  if (commit.stat) return `${plural(commit.stat.files, "file")} · +${commit.stat.added} −${commit.stat.removed}`;
  return commit.subject;
}

/** The tooltip of a graph line: the branch it belongs to, or what the row is. */
export function lineName(commit: Commit, name: string | null) {
  if (commit.id === WORKTREE_ID) return "uncommitted changes";
  if (commit.shelved) return `WIP saved from ${commit.shelved}`;
  return name;
}

/** A logged git command as a terminal line: arguments with spaces or shell characters quoted, so it can be pasted. */
export const gitCommand = (l: GitLog) => "git " + l.args.map((a) => (/[\s"'$`\\|&;<>()*?]/.test(a) ? `'${a.replace(/'/g, `'\\''`)}'` : a)).join(" ");

/** The commands an action ran: same repo, from when it started until it was recorded (plus a moment for the last). */
export const ranBy = (l: GitLog, action: { repo: string; started?: number; at: number }) =>
  action.started != null && l.repo === action.repo && l.at >= action.started && l.at <= action.at + 500;
