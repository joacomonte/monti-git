//! Code editors the user has installed, for "Open in …": found by app bundle on macOS, by command on PATH elsewhere.
//! They open the repo folder with the file in front, so the whole project is at hand.
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
pub struct Editor {
    pub id: &'static str,
    pub name: &'static str,
}

struct Known {
    id: &'static str,
    name: &'static str,
    bundles: &'static [&'static str], // macOS, in /Applications or ~/Applications (JetBrains Toolbox)
    command: &'static str,            // Windows and Linux, on PATH
}

const KNOWN: &[Known] = &[
    Known { id: "vscode", name: "VS Code", bundles: &["Visual Studio Code.app"], command: "code" },
    Known { id: "cursor", name: "Cursor", bundles: &["Cursor.app"], command: "cursor" },
    Known { id: "zed", name: "Zed", bundles: &["Zed.app"], command: "zed" },
    Known { id: "sublime", name: "Sublime Text", bundles: &["Sublime Text.app"], command: "subl" },
    Known {
        id: "intellij",
        name: "IntelliJ IDEA",
        bundles: &["IntelliJ IDEA.app", "IntelliJ IDEA Ultimate.app", "IntelliJ IDEA CE.app", "IntelliJ IDEA Community Edition.app"],
        command: "idea",
    },
    Known { id: "webstorm", name: "WebStorm", bundles: &["WebStorm.app"], command: "webstorm" },
];

/// Where `editor` is installed: its app bundle (macOS) or its command (elsewhere).
fn locate(editor: &Known) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let dirs = [Some(PathBuf::from("/Applications")), home.map(|h| h.join("Applications"))];
        return dirs.iter().flatten().flat_map(|dir| editor.bundles.iter().map(move |b| dir.join(b))).find(|p| p.exists());
    }
    let names: &[&str] = if cfg!(windows) { &["exe", "cmd", "bat"] } else { &[""] };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |ext| dir.join(editor.command).with_extension(ext)))
        .find(|p| p.is_file())
}

pub fn installed() -> Vec<Editor> {
    KNOWN.iter().filter(|e| locate(e).is_some()).map(|e| Editor { id: e.id, name: e.name }).collect()
}

pub fn open(id: &str, repo: &Path, file: &Path) -> Result<(), String> {
    let editor = KNOWN.iter().find(|e| e.id == id).ok_or_else(|| format!("unknown editor {id}"))?;
    let found = locate(editor).ok_or_else(|| format!("{} isn't installed anymore", editor.name))?;
    let mut command = if cfg!(target_os = "macos") {
        let mut open = Command::new("open");
        open.arg("-a").arg(found);
        open
    } else if cfg!(windows) {
        let mut cmd = Command::new("cmd"); // code.cmd and friends are scripts: they run through cmd
        cmd.arg("/C").arg(found);
        cmd
    } else {
        Command::new(found)
    };
    command.arg(repo).arg(file);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
    }
    let mut child = command.spawn().map_err(|e| format!("couldn't start {}: {e}", editor.name))?;
    std::thread::spawn(move || child.wait()); // reaped when it exits, without holding the command
    Ok(())
}
