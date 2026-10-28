# monti-git

Friendly git dashboard and graph for Mac and Windows (Tauri + SvelteKit + TypeScript).

## Install

Download the installer from the [latest release](https://github.com/joacomonte/monti-git/releases/latest):
- Mac: the `universal.dmg` (Apple Silicon and Intel). The first time, right click → Open (the app isn't notarized).
- Windows: the `-setup.exe`. If SmartScreen warns, "More info" → "Run anyway".

After that it updates itself: at launch it installs any new version and restarts.

## Dev

```bash
pnpm install
pnpm tauri dev
```

## Release

1. Bump `version` in `src-tauri/tauri.conf.json` (e.g. `0.2.0`).
   Add its entry at the top of `src/lib/changelog.json`: the app shows it after updating, and it becomes the release text.
2. Commit, then `git tag v0.2.0 && git push origin main v0.2.0`.
3. The `release` workflow creates a draft release, builds Mac + Windows in parallel, then writes `latest.json` and publishes it.

The updater signing key lives outside the repo (`/Users/monti/.config/monti-access/monti-git-updater.key`) and in the
repo secret `TAURI_SIGNING_PRIVATE_KEY`. If it's lost, installed apps can't update anymore: they need a manual reinstall.
