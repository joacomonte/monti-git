// Branch names as git prints them in decorations: "HEAD -> main", "origin/main", "tag: v1". Same rule as src-tauri/src/refs.rs.

const REMOTE = /^(origin|upstream)\//;

/** "origin/x" / "upstream/x" -> "x"; anything else as is. */
export const localName = (ref: string) => ref.replace(REMOTE, "");

export const isRemote = (ref: string) => REMOTE.test(ref);

/** A decoration without its "HEAD -> " marker. */
export const refName = (decoration: string) => decoration.replace(/^HEAD -> /, "");

export const isTag = (decoration: string) => decoration.startsWith("tag: ");

export const isHead = (decoration: string) => decoration.startsWith("HEAD -> ");

/** One badge per branch of a commit's decorations: a remote at the same commit as its local branch folds into it
 *  (`isSynced`: pushed, nothing to push or pull). Tags stay as they are. */
export function branchBadges(refs: string[]): { ref: string; isSynced: boolean }[] {
  const locals = new Set(refs.map(refName));
  const folded = new Set(refs.filter(isRemote).map(localName).filter((n) => locals.has(n)));
  return refs.flatMap((ref) => {
    if (isRemote(ref)) return localName(ref) === "HEAD" || folded.has(localName(ref)) ? [] : [{ ref, isSynced: false }];
    return [{ ref, isSynced: folded.has(refName(ref)) }];
  });
}

/** Local branches at this commit that the server doesn't have here: never pushed, or with commits to push. */
export const unpushed = (refs: string[]) =>
  branchBadges(refs).filter((b) => !b.isSynced && !isRemote(b.ref) && !isTag(b.ref)).map((b) => refName(b.ref));

/** The amber chip of a branch that the server doesn't have at this commit. */
export const LOCAL_COMMITTED = {
  label: "Local committed",
  title: "Committed on this computer, not pushed yet (or the server has a different version).",
} as const;
