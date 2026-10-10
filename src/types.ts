export interface RepoInfo {
  path: string;
  name: string;
  head: string | null;
  is_detached: boolean;
  state: string;
  ahead: number;
  behind: number;
  upstream: string | null;
  is_unborn: boolean;
  remotes: string[];
  uses_lfs: boolean;
  lfs_installed: boolean;
}

export interface FileChange {
  path: string;
  old_path: string | null;
  staged: string | null;
  unstaged: string | null;
  conflicted: boolean;
}

export interface RefLabel {
  name: string;
  kind: "branch" | "remote" | "tag" | string;
}

export interface CommitInfo {
  oid: string;
  short_id: string;
  summary: string;
  body: string;
  author_name: string;
  author_email: string;
  committer_name: string;
  author_time: number;
  committer_time: number;
  parents: string[];
  refs: RefLabel[];
}

export interface ChangedFile {
  path: string;
  old_path: string | null;
  status: string;
  additions: number;
  deletions: number;
}

export interface CommitDetail {
  commit: CommitInfo;
  files: ChangedFile[];
}

export interface DiffLine {
  kind: "add" | "del" | "context" | string;
  old_lineno: number | null;
  new_lineno: number | null;
  content: string;
}

export interface DiffHunk {
  header: string;
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  old_path: string | null;
  status: string;
  is_binary: boolean;
  too_large: boolean;
  additions: number;
  deletions: number;
  hunks: DiffHunk[];
}

export interface BranchInfo {
  name: string;
  is_head: boolean;
  is_remote: boolean;
  upstream: string | null;
  ahead: number | null;
  behind: number | null;
  last_commit_oid: string | null;
  last_commit_summary: string;
  last_commit_time: number;
}

export interface RemoteInfo {
  name: string;
  url: string | null;
  push_url: string | null;
}

export interface StashInfo {
  index: number;
  message: string;
  oid: string;
  time: number;
}

export interface StashFileInfo {
  path: string;
  status: string;
}

export interface TagInfo {
  name: string;
  oid: string;
  annotated: boolean;
  message: string | null;
  time: number;
}

export interface MergeResult {
  status: "up_to_date" | "fast_forward" | "merged" | "conflicts" | "unborn" | string;
  conflicts: string[];
}

export interface OpProgress {
  op: string;
  received: number;
  total: number;
  path: string;
}

export interface BlameHunkInfo {
  commit_oid: string;
  short_id: string;
  author: string;
  start_line: number;
  line_count: number;
  summary: string;
}

export interface SubmoduleInfo {
  name: string;
  path: string;
  url: string | null;
}

export interface ImageDiff {
  old: string | null;
  new: string | null;
  mime: string;
}

export interface GitConfigInfo {
  name: string | null;
  email: string | null;
}

export interface RecentRepos {
  recent: string[];
  last: string | null;
}

export interface CredentialStatus {
  username: string | null;
  has_password: boolean;
  remembered: boolean;
}

export interface GhRepo {
  owner: string;
  name: string;
  url: string;
}

export interface GhUser {
  login: string;
  name: string | null;
  avatar_url: string | null;
}

export interface GhPR {
  number: number;
  title: string;
  author: string;
  avatar_url: string | null;
  head_ref: string;
  head_sha: string;
  base_ref: string;
  draft: boolean;
  html_url: string;
  updated_at: string;
}

export interface GhLabel {
  name: string;
  color: string;
}

export interface GhIssue {
  number: number;
  title: string;
  author: string;
  avatar_url: string | null;
  labels: GhLabel[];
  comments: number;
  html_url: string;
  created_at: string;
}

export interface GhCheckStatus {
  status: "success" | "failure" | "pending" | "none" | string;
  total: number;
  failed: number;
}

export interface DeviceFlow {
  device_code: string;
  user_code: string;
  verification_uri: string;
  interval: number;
}

export interface DevicePoll {
  status: "pending" | "slow_down" | "expired" | "authorized" | string;
  token: string | null;
}
