// Release helpers for .github/workflows/release.yml (run from the repo root, with GITHUB_REF_NAME = "v0.1.6"):
//   node .github/scripts/release.mjs notes        → release text from src/lib/changelog.json
//   node .github/scripts/release.mjs latest-json  → updater manifest from the .sig files in ./sigs
// The build jobs run in parallel and don't upload latest.json themselves, so they can't race to rewrite it.
import { readFileSync, readdirSync } from "node:fs";

const tag = process.env.GITHUB_REF_NAME;
const repo = process.env.GITHUB_REPOSITORY;
const changelog = JSON.parse(readFileSync("src/lib/changelog.json", "utf8"));
const changes = changelog.find((r) => "v" + r.version === tag)?.changes ?? [];
// Every new feature ships with an animated demo in "What's new" (src/lib/components/WhatsNewDemo.svelte).
const undemoed = changes.filter((c) => c.kind === "new" && !c.demo).map((c) => c.title);
if (undemoed.length) throw new Error(`"new" changes without a "demo" in changelog.json: ${undemoed.join(", ")}`);
const notes = changes.map((c) => `- **${c.title}** (${c.kind}): ${c.detail}`).join("\n");

if (process.argv[2] === "notes") {
  console.log(`${notes}\n\nMac: the .dmg (Apple Silicon and Intel). Windows: the -setup.exe. Installed apps update themselves at launch.`);
} else if (process.argv[2] === "latest-json") {
  const platforms = {};
  for (const sig of readdirSync("sigs")) {
    const asset = sig.slice(0, -".sig".length);
    const entry = { signature: readFileSync(`sigs/${sig}`, "utf8").trim(), url: `https://github.com/${repo}/releases/download/${tag}/${asset}` };
    // Universal Mac build: one archive for both architectures. Windows defaults to the -setup.exe installer.
    if (asset.endsWith(".app.tar.gz")) for (const key of ["darwin-aarch64", "darwin-x86_64", "darwin-aarch64-app", "darwin-x86_64-app"]) platforms[key] = entry;
    else if (asset.endsWith("-setup.exe")) platforms["windows-x86_64"] = platforms["windows-x86_64-nsis"] = entry;
    else if (asset.endsWith(".msi")) platforms["windows-x86_64-msi"] = entry;
  }
  if (!platforms["darwin-aarch64"] || !platforms["windows-x86_64"]) throw new Error(`missing updater files, found: ${Object.keys(platforms)}`);
  console.log(JSON.stringify({ version: tag.replace(/^v/, ""), notes, pub_date: new Date().toISOString(), platforms }, null, 2));
} else {
  throw new Error("usage: release.mjs notes | latest-json");
}
