// What the app remembers between launches (localStorage), by key. Keys keep the Swift app's UserDefaults names.
import type { CodeScope } from "./api";
import type { Activity } from "./model.svelte";

export type Sources = { folders: string[]; repos: string[]; hidden: string[] }; // hidden: repos of an added folder taken out of the list
export type SearchMode = "commits" | "code" | "both";

export type TreeSearchMode = "names" | "lines" | "both";

type Prefs = {
  lastRepo: string | null;
  onlyMine: boolean;
  branchesOnly: boolean;
  autoSync: boolean;
  mainBranches: Record<string, string>; // repo -> the base the user picked
  environments: Record<string, string[]>; // repo -> the environments the user picked
  repoSources: Sources;
  sidebarPrefs: { favorites?: string[] };
  activity: Activity[];
  treeShowsAll: boolean;
  treeFlat: boolean;
  treeWidth: number;
  diffHeight: number;
  treeSearchMode: TreeSearchMode;
  treeSearchDiffOnly: boolean;
  searchScope: "repo" | "all";
  searchMode: SearchMode;
  searchCodeWhere: CodeScope;
  commitDetailsHidden: string[];
  actionToastFolded: boolean;
  lastSeenVersion: string | null;
  notesRead: string | null;
};

export const prefs = {
  /** The stored value, or `fallback` when there is none or it can't be read (never throws). */
  get<K extends keyof Prefs>(key: K, fallback: Prefs[K]): Prefs[K] {
    try {
      const raw = localStorage.getItem(key);
      return raw == null ? fallback : (JSON.parse(raw) as Prefs[K]);
    } catch {
      return fallback;
    }
  },
  set<K extends keyof Prefs>(key: K, value: Prefs[K]) {
    localStorage.setItem(key, JSON.stringify(value));
  },
};
