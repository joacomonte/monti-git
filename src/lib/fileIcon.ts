// File type icons from the VS Code Material Icon Theme: its manifest maps file names and extensions to SVGs.
import manifest from "material-icon-theme/dist/material-icons.json";

type Manifest = {
  file: string;
  fileNames: Record<string, string>;
  fileExtensions: Record<string, string>;
  iconDefinitions: Record<string, { iconPath: string }>;
};
const { file, fileNames, fileExtensions, iconDefinitions } = manifest as unknown as Manifest;

// Every file icon as a bundled asset URL (folder icons left out: the tree keeps its own folder glyph).
const DIR = "/node_modules/material-icon-theme/icons/";
const urls = import.meta.glob(["/node_modules/material-icon-theme/icons/*.svg", "!**/folder*.svg"], {
  query: "?url",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** "src/app/user.service.ts" → URL of its icon. Exact names first (package.json), then the longest extension (d.ts before ts). */
export function fileIcon(path: string): string {
  const name = path.slice(path.lastIndexOf("/") + 1).toLowerCase();
  let icon = fileNames[name];
  for (let i = name.indexOf("."); !icon && i >= 0; i = name.indexOf(".", i + 1)) icon = fileExtensions[name.slice(i + 1)];
  const svg = (iconDefinitions[icon ?? file] ?? iconDefinitions[file]).iconPath.split("/").pop();
  return urls[DIR + svg] ?? urls[DIR + "file.svg"];
}
