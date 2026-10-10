use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub struct AppState {
    pub repo_path: Mutex<Option<PathBuf>>,
    pub creds: Mutex<Credentials>,
    pub watcher: Mutex<Option<RecommendedWatcher>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            repo_path: Mutex::new(None),
            creds: Mutex::new(load_credentials()),
            watcher: Mutex::new(None),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct PersistedState {
    pub recent: Vec<String>,
    pub last_repo: Option<String>,
    pub credentials: Option<Credentials>,
}

fn state_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("git-crackit").join("state.json"))
}

pub fn load_persisted() -> PersistedState {
    state_file()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_persisted(st: &PersistedState) {
    if let Some(p) = state_file() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, serde_json::to_string_pretty(st).unwrap_or_default());
    }
}

pub fn add_recent(path: &str) {
    let mut st = load_persisted();
    st.recent.retain(|p| p != path);
    st.recent.insert(0, path.to_string());
    st.recent.truncate(15);
    st.last_repo = Some(path.to_string());
    save_persisted(&st);
}

pub fn remove_recent(path: &str) {
    let mut st = load_persisted();
    st.recent.retain(|p| p != path);
    if st.last_repo.as_deref() == Some(path) {
        st.last_repo = st.recent.first().cloned();
    }
    save_persisted(&st);
}

pub fn set_last(path: Option<String>) {
    let mut st = load_persisted();
    st.last_repo = path;
    save_persisted(&st);
}

/// Persist (or clear, with `None`) remembered HTTPS credentials.
/// Prefers the OS keychain; falls back to state.json when unavailable.
pub fn save_credentials(creds: Option<Credentials>) {
    let mut st = load_persisted();
    match creds {
        Some(c) => {
            let in_keyring = keyring::Entry::new("git-crackit", "https")
                .ok()
                .and_then(|e| {
                    e.set_password(&serde_json::to_string(&c).unwrap_or_default())
                        .ok()
                })
                .is_some();
            // Plaintext only when keyring is unavailable.
            st.credentials = if in_keyring { None } else { Some(c) };
        }
        None => {
            if let Ok(e) = keyring::Entry::new("git-crackit", "https") {
                let _ = e.delete_credential();
            }
            st.credentials = None;
        }
    }
    save_persisted(&st);
}

/// Load remembered credentials: OS keychain first, then state.json fallback
/// (migrating plaintext to the keychain when possible).
pub fn load_credentials() -> Credentials {
    if let Ok(e) = keyring::Entry::new("git-crackit", "https") {
        if let Ok(s) = e.get_password() {
            if let Ok(c) = serde_json::from_str::<Credentials>(&s) {
                return c;
            }
        }
    }
    if let Some(c) = load_persisted().credentials {
        let c2 = c.clone();
        std::thread::spawn(move || save_credentials(Some(c2))); // migrate to keyring
        return c;
    }
    Credentials::default()
}

/// Whether remembered credentials exist (keychain or file fallback).
pub fn credentials_remembered() -> bool {
    keyring::Entry::new("git-crackit", "https")
        .ok()
        .and_then(|e| e.get_password().ok())
        .is_some()
        || load_persisted().credentials.is_some()
}
