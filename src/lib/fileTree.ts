// "a/b/c.ts" paths -> nested nodes, folders first, then files, each sorted Finder-style.

export type FileNode = { name: string; path: string; children: FileNode[] | null };

const byName = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" }).compare;

export function fileTree(paths: Iterable<string>, prefix = ""): FileNode[] {
  const files: string[] = [];
  const folders = new Map<string, string[]>();
  for (const path of paths) {
    const slash = path.indexOf("/");
    if (slash < 0) {
      files.push(path);
      continue;
    }
    const name = path.slice(0, slash);
    const inside = folders.get(name);
    if (inside) inside.push(path.slice(slash + 1));
    else folders.set(name, [path.slice(slash + 1)]);
  }
  const dirs = [...folders.keys()].sort(byName).map((name) => {
    let node: FileNode = { name, path: prefix + name, children: fileTree(folders.get(name)!, prefix + name + "/") };
    // Folders whose only child is a folder collapse into one row: "src/main/java".
    while (node.children?.length === 1 && node.children[0].children) {
      const only = node.children[0];
      node = { name: node.name + "/" + only.name, path: only.path, children: only.children };
    }
    return node;
  });
  return [...dirs, ...files.sort(byName).map((name) => ({ name, path: prefix + name, children: null }))];
}

/** The folders above `path`: "a/b/c.ts" -> "a", "a/b". */
export function parentDirs(path: string) {
  const parts = path.split("/").slice(0, -1);
  return parts.map((_, i) => parts.slice(0, i + 1).join("/"));
}

/** Files in tree order (folders first at every level). */
export const filesInOrder = (nodes: FileNode[]): string[] => nodes.flatMap((n) => (n.children ? filesInOrder(n.children) : [n.path]));
