use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub head: Option<String>,
    pub is_detached: bool,
    pub state: String,
    pub ahead: usize,
    pub behind: usize,
    pub upstream: Option<String>,
    pub is_unborn: bool,
    pub remotes: Vec<String>,
    pub uses_lfs: bool,
    pub lfs_installed: bool,
}

#[derive(Serialize)]
pub struct FileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub staged: Option<String>,
    pub unstaged: Option<String>,
    pub conflicted: bool,
}

#[derive(Serialize, Clone)]
pub struct RefLabel {
    pub name: String,
    pub kind: String,
}

#[derive(Serialize)]
pub struct CommitInfo {
    pub oid: String,
    pub short_id: String,
    pub summary: String,
    pub body: String,
    pub author_name: String,
    pub author_email: String,
    pub committer_name: String,
    pub author_time: i64,
    pub committer_time: i64,
    pub parents: Vec<String>,
    pub refs: Vec<RefLabel>,
}

#[derive(Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Serialize)]
pub struct CommitDetail {
    pub commit: CommitInfo,
    pub files: Vec<ChangedFile>,
}

#[derive(Serialize)]
pub struct DiffLine {
    pub kind: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

#[derive(Serialize)]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub is_binary: bool,
    pub too_large: bool,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    pub last_commit_oid: Option<String>,
    pub last_commit_summary: String,
    pub last_commit_time: i64,
}

#[derive(Serialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: Option<String>,
    pub push_url: Option<String>,
}

#[derive(Serialize)]
pub struct StashInfo {
    pub index: usize,
    pub message: String,
    pub oid: String,
    pub time: i64,
}

#[derive(Serialize)]
pub struct TagInfo {
    pub name: String,
    pub oid: String,
    pub annotated: bool,
    pub message: Option<String>,
    pub time: i64,
}

#[derive(Serialize)]
pub struct MergeResult {
    pub status: String, // "up_to_date" | "fast_forward" | "merged" | "conflicts" | "unborn"
    pub conflicts: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct OpProgress {
    pub op: String,
    pub received: usize,
    pub total: usize,
    pub path: String,
}

#[derive(Serialize)]
pub struct BlameHunkInfo {
    pub commit_oid: String,
    pub short_id: String,
    pub author: String,
    pub start_line: usize,
    pub line_count: usize,
    pub summary: String,
}

#[derive(Serialize)]
pub struct SubmoduleInfo {
    pub name: String,
    pub path: String,
    pub url: Option<String>,
}

#[derive(Serialize)]
pub struct ImageDiff {
    pub old: Option<String>,
    pub new: Option<String>,
    pub mime: String,
}

#[derive(Serialize)]
pub struct GitConfigInfo {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Serialize)]
pub struct RecentRepos {
    pub recent: Vec<String>,
    pub last: Option<String>,
}

#[derive(Serialize)]
pub struct CredentialStatus {
    pub username: Option<String>,
    pub has_password: bool,
    pub remembered: bool,
}

#[derive(Serialize, Clone)]
pub struct GhRepo {
    pub owner: String,
    pub name: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct GhUser {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Serialize)]
pub struct GhPR {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub avatar_url: Option<String>,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    pub draft: bool,
    pub html_url: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct GhLabel {
    pub name: String,
    pub color: String,
}

#[derive(Serialize)]
pub struct GhIssue {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub avatar_url: Option<String>,
    pub labels: Vec<GhLabel>,
    pub comments: u64,
    pub html_url: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct GhCheckStatus {
    /// "success" | "failure" | "pending" | "none"
    pub status: String,
    pub total: usize,
    pub failed: usize,
}

#[derive(Serialize)]
pub struct DeviceFlow {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

#[derive(Serialize)]
pub struct DevicePoll {
    /// "pending" | "slow_down" | "expired" | "authorized"
    pub status: String,
    pub token: Option<String>,
}
