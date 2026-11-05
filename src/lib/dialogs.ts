// The native folder picker, for adding repos.
import { open } from "@tauri-apps/plugin-dialog";

/** "repo": one or more git repos. "folder": a folder whose repos are all listed. Empty if cancelled. */
export async function pickFolders(kind: "repo" | "folder"): Promise<string[]> {
  const picked = await open({ directory: true, multiple: true, title: kind === "repo" ? "Add repositories" : "Add a folder of repositories" });
  return picked == null ? [] : Array.isArray(picked) ? picked : [picked];
}
