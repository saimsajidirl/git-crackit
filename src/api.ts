import { invoke } from "@tauri-apps/api/core";
import type {
  RepoInfo, FileChange, CommitInfo, CommitDetail, FileDiff, BranchInfo,
  RemoteInfo, StashInfo, StashFileInfo, TagInfo, MergeResult, BlameHunkInfo, SubmoduleInfo,
  GitConfigInfo, RecentRepos, CredentialStatus,
  GhRepo, GhUser, GhPR, GhIssue, GhCheckStatus, DeviceFlow, DevicePoll, ImageDiff, AiSettingsInfo,
  CheckoutOutcome, CompareResult,
} from "./types";

export const api = {
  openRepository: (path: string) => invoke<RepoInfo>("open_repository", { path }),
  initRepository: (path: string) => invoke<RepoInfo>("init_repository", { path }),
  cloneRepository: (url: string, dest: string) => invoke<RepoInfo>("clone_repository", { url, dest }),
  closeRepository: () => invoke<void>("close_repository"),
  repositoryInfo: () => invoke<RepoInfo>("repository_info"),
  getRecentRepositories: () => invoke<RecentRepos>("get_recent_repositories"),
  removeRecentRepository: (path: string) => invoke<void>("remove_recent_repository", { path }),
  validateRepositoryPath: (path: string) => invoke<boolean>("validate_repository_path", { path }),
  setCredentials: (username: string | null, password: string | null, remember: boolean) =>
    invoke<void>("set_credentials", { username, password, remember }),
  getCredentials: () => invoke<CredentialStatus>("get_credentials"),
  clearCredentials: () => invoke<void>("clear_credentials"),
  listSshKeys: () => invoke<string[]>("list_ssh_keys"),
  loadSshKey: (path: string, passphrase: string) => invoke<string>("load_ssh_key", { path, passphrase }),
  getAiSettings: () => invoke<AiSettingsInfo>("get_ai_settings"),
  setAiSettings: (provider: string, model: string, baseUrl: string, apiKey: string | null) =>
    invoke<void>("set_ai_settings", { provider, model, baseUrl, apiKey }),
  listOllamaModels: () => invoke<string[]>("list_ollama_models"),
  generateCommitMessage: () => invoke<string>("generate_commit_message"),
  openExternalUrl: (url: string) => invoke<void>("open_external_url", { url }),

  getStatus: () => invoke<FileChange[]>("get_status"),
  stageFiles: (paths: string[]) => invoke<void>("stage_files", { paths }),
  unstageFiles: (paths: string[]) => invoke<void>("unstage_files", { paths }),
  stageAll: () => invoke<void>("stage_all"),
  unstageAll: () => invoke<void>("unstage_all"),
  discardChanges: (paths: string[]) => invoke<void>("discard_changes", { paths }),
  stageHunk: (path: string, hunk: number) => invoke<void>("stage_hunk", { path, hunk }),
  unstageHunk: (path: string, hunk: number) => invoke<void>("unstage_hunk", { path, hunk }),
  discardHunk: (path: string, hunk: number) => invoke<void>("discard_hunk", { path, hunk }),
  createCommit: (message: string, amend?: boolean) =>
    invoke<string>("create_commit", { message, amend }),
  getWorkingDiff: (path: string, staged: boolean) =>
    invoke<FileDiff>("get_working_diff", { path, staged }),
  getImageDiff: (path: string, staged: boolean) =>
    invoke<ImageDiff>("get_image_diff", { path, staged }),
  listSubmodules: () => invoke<SubmoduleInfo[]>("list_submodules"),
  updateSubmodules: () => invoke<void>("update_submodules"),
  ignoreFile: (path: string) => invoke<void>("ignore_file", { path }),
  getBlame: (path: string) => invoke<BlameHunkInfo[]>("get_blame", { path }),
  getGitIdentity: () => invoke<GitConfigInfo>("get_git_identity"),
  setGitIdentity: (name: string, email: string, global: boolean) =>
    invoke<void>("set_git_identity", { name, email, global }),
  setSigningConfig: (gpgsign: boolean, signingkey: string | null, format: string, global: boolean) =>
    invoke<void>("set_signing_config", { gpgsign, signingkey, format, global }),

  getHistory: (skip?: number, limit?: number, query?: string, refname?: string) =>
    invoke<CommitInfo[]>("get_history", { skip, limit, query, refname }),
  getCommitDetail: (oid: string) => invoke<CommitDetail>("get_commit_detail", { oid }),
  getCommitFileDiff: (oid: string, path: string) =>
    invoke<FileDiff>("get_commit_file_diff", { oid, path }),
  cherryPick: (oid: string) => invoke<MergeResult>("cherry_pick", { oid }),
  revertCommit: (oid: string) => invoke<MergeResult>("revert_commit", { oid }),
  resetToCommit: (oid: string, mode: "soft" | "mixed" | "hard") =>
    invoke<void>("reset_to_commit", { oid, mode }),
  checkoutCommit: (oid: string, dirty?: "carry" | "stash") =>
    invoke<CheckoutOutcome>("checkout_commit", { oid, dirty }),
  undoLast: () => invoke<string>("undo_last"),
  redoLast: () => invoke<string>("redo_last"),
  fileHistory: (path: string) => invoke<CommitInfo[]>("file_history", { path }),
  compareRefs: (base: string, compare: string) =>
    invoke<CompareResult>("compare_refs", { base, compare }),
  compareFileDiff: (base: string, compare: string, path: string) =>
    invoke<FileDiff>("compare_file_diff", { base, compare, path }),

  listBranches: () => invoke<BranchInfo[]>("list_branches"),
  createBranch: (name: string, startPoint?: string, checkout?: boolean, dirty?: "carry" | "stash") =>
    invoke<CheckoutOutcome>("create_branch", { name, startPoint, checkout, dirty }),
  checkoutBranch: (name: string, dirty?: "carry" | "stash") =>
    invoke<CheckoutOutcome>("checkout_branch", { name, dirty }),
  deleteBranch: (name: string) => invoke<void>("delete_branch", { name }),
  renameBranch: (old: string, newName: string) =>
    invoke<void>("rename_branch", { old, new: newName }),
  mergeBranch: (name: string, dirty?: "carry" | "stash") =>
    invoke<CheckoutOutcome>("merge_branch", { name, dirty }),
  abortMerge: () => invoke<void>("abort_merge"),
  rebaseBranch: (onto: string, dirty?: "carry" | "stash") =>
    invoke<CheckoutOutcome>("rebase_branch", { onto, dirty }),
  rebaseCommitAction: (oid: string, target: string, action: "move_after" | "squash_into") =>
    invoke<MergeResult>("rebase_commit_action", { oid, target, action }),

  listRemotes: () => invoke<RemoteInfo[]>("list_remotes"),
  addRemote: (name: string, url: string) => invoke<void>("add_remote", { name, url }),
  removeRemote: (name: string) => invoke<void>("remove_remote", { name }),
  renameRemote: (old: string, newName: string) =>
    invoke<void>("rename_remote", { old, new: newName }),
  setRemoteUrl: (name: string, url: string) => invoke<void>("set_remote_url", { name, url }),
  fetchRemote: (remote?: string) => invoke<void>("fetch_remote", { remote }),
  fetchAll: () => invoke<void>("fetch_all"),
  pull: (dirty?: "carry" | "stash") => invoke<CheckoutOutcome>("pull", { dirty }),
  push: (setUpstream?: boolean, force?: boolean) =>
    invoke<void>("push", { setUpstream, force }),
  pushTag: (name: string, remote?: string) => invoke<void>("push_tag", { name, remote }),
  setBranchUpstream: (branch: string, upstream: string) =>
    invoke<void>("set_branch_upstream", { branch, upstream }),
  fetchPrBranch: (number: number) => invoke<string>("fetch_pr_branch", { number }),

  githubRepo: () => invoke<GhRepo | null>("github_repo"),
  githubUser: () => invoke<GhUser>("github_user"),
  githubPrs: () => invoke<GhPR[]>("github_prs"),
  githubIssues: () => invoke<GhIssue[]>("github_issues"),
  githubCheckRuns: (sha: string) => invoke<GhCheckStatus>("github_check_runs", { sha }),
  githubDeviceStart: () => invoke<DeviceFlow>("github_device_start"),
  githubDevicePoll: (deviceCode: string) => invoke<DevicePoll>("github_device_poll", { deviceCode }),

  listStashes: () => invoke<StashInfo[]>("list_stashes"),
  stashFiles: (index: number) => invoke<StashFileInfo[]>("stash_files", { index }),
  stashFileDiff: (index: number, path: string) =>
    invoke<FileDiff>("stash_file_diff", { index, path }),
  stashSave: (message: string, includeUntracked: boolean) =>
    invoke<void>("stash_save", { message, includeUntracked }),
  stashApply: (index: number) => invoke<MergeResult>("stash_apply", { index }),
  stashPop: (index: number) => invoke<void>("stash_pop", { index }),
  stashDrop: (index: number) => invoke<void>("stash_drop", { index }),
  listTags: () => invoke<TagInfo[]>("list_tags"),
  createTag: (name: string, target?: string, message?: string) =>
    invoke<void>("create_tag", { name, target, message }),
  deleteTag: (name: string) => invoke<void>("delete_tag", { name }),
};
