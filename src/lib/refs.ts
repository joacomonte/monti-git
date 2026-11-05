// Branch names as git prints them in decorations: "HEAD -> main", "origin/main", "tag: v1". Same rule as src-tauri/src/refs.rs.

const REMOTE = /^(origin|upstream)\//;

/** "origin/x" / "upstream/x" -> "x"; anything else as is. */
export const localName = (ref: string) => ref.replace(REMOTE, "");

export const isRemote = (ref: string) => REMOTE.test(ref);

/** A decoration without its "HEAD -> " marker. */
export const refName = (decoration: string) => decoration.replace(/^HEAD -> /, "");

export const isTag = (decoration: string) => decoration.startsWith("tag: ");

export const isHead = (decoration: string) => decoration.startsWith("HEAD -> ");
