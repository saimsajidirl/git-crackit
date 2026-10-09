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
            creds: Mutex::new(load_persisted().credentials.unwrap_or_default()),
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
pub fn save_credentials(creds: Option<Credentials>) {
    let mut st = load_persisted();
    st.credentials = creds;
    save_persisted(&st);
}
