use crate::state::AppState;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};

/// Start watching `root` (repo workdir). Emits:
///   "repo-changed" with payload "workdir" (file edits) or "git" (.git metadata).
/// Replaces any existing watcher.
pub fn start<R: Runtime>(app: &AppHandle<R>, state: &AppState, root: &Path) {
    stop(state);

    let (tx, rx) = channel::<&'static str>();
    let watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(ev) = res {
            let in_git = ev
                .paths
                .iter()
                .any(|p| p.components().any(|c| c.as_os_str() == ".git"));
            let _ = tx.send(if in_git { "git" } else { "workdir" });
        }
    });

    let mut watcher: RecommendedWatcher = match watcher {
        Ok(w) => w,
        Err(_) => return,
    };
    if watcher.watch(root, RecursiveMode::Recursive).is_err() {
        return;
    }

    // Debounce: coalesce bursts for 350ms, "git" wins over "workdir".
    let app2 = app.clone();
    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            let mut kind = first;
            let deadline = Instant::now() + Duration::from_millis(350);
            loop {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                match rx.recv_timeout(deadline - now) {
                    Ok(k) => {
                        if k == "git" {
                            kind = "git";
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            let _ = app2.emit("repo-changed", kind);
        }
    });

    if let Ok(mut guard) = state.watcher.lock() {
        *guard = Some(watcher);
    }
}

/// Stop watching (e.g., when the repo is closed).
pub fn stop(state: &AppState) {
    if let Ok(mut guard) = state.watcher.lock() {
        guard.take(); // dropping the watcher stops it
    }
}
