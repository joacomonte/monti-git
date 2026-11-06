//! The main window opens where and as big as it was left: its frame is saved to the app's config folder on every move
//! or resize, and put back before the window first shows (it starts hidden, so there's no jump from the default size).
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Frame {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("window.json"))
}

/// Puts the saved frame back (the position only if it's still on a connected screen), then shows the window.
pub fn restore(window: &WebviewWindow) {
    let saved = file(window.app_handle()).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|s| serde_json::from_str::<Frame>(&s).ok());
    if let Some(frame) = saved {
        let _ = window.set_size(PhysicalSize::new(frame.width, frame.height));
        let on_screen = window.available_monitors().unwrap_or_default().iter().any(|m| {
            let (p, s) = (m.position(), m.size());
            frame.x >= p.x && frame.y >= p.y && frame.x < p.x + s.width as i32 && frame.y < p.y + s.height as i32
        });
        if on_screen {
            let _ = window.set_position(PhysicalPosition::new(frame.x, frame.y));
        }
        if frame.maximized {
            let _ = window.maximize();
        }
    }
    let _ = window.show();
}

/// Saves the current frame. While maximized only the flag changes, so un-maximizing returns to the last normal frame.
pub fn save(window: &tauri::Window) {
    let (Ok(pos), Ok(size), Some(path)) = (window.outer_position(), window.inner_size(), file(window.app_handle())) else { return };
    let maximized = window.is_maximized().unwrap_or(false);
    let previous = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Frame>(&s).ok());
    let frame = match previous {
        Some(prev) if maximized => Frame { maximized: true, ..prev },
        _ => Frame { x: pos.x, y: pos.y, width: size.width, height: size.height, maximized },
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(&frame) {
        let _ = std::fs::write(path, text);
    }
}
