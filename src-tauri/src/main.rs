#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cmds_branches;
mod cmds_files;
mod cmds_history;
mod cmds_remotes;
mod cmds_repo;
mod cmds_stash_tags;
mod helpers;
mod state;
mod types;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // repo lifecycle
            cmds_repo::open_repository,
            cmds_repo::init_repository,
            cmds_repo::clone_repository,
            cmds_repo::close_repository,
            cmds_repo::repository_info,
            cmds_repo::get_recent_repositories,
            cmds_repo::remove_recent_repository,
            cmds_repo::validate_repository_path,
            cmds_repo::set_credentials,
            // working tree / files
            cmds_files::get_status,
            cmds_files::stage_files,
            cmds_files::unstage_files,
            cmds_files::stage_all,
            cmds_files::unstage_all,
            cmds_files::discard_changes,
            cmds_files::create_commit,
            cmds_files::get_working_diff,
            cmds_files::list_submodules,
            cmds_files::get_blame,
            cmds_files::get_git_identity,
            cmds_files::set_git_identity,
            // history / commits
            cmds_history::get_history,
            cmds_history::get_commit_detail,
            cmds_history::get_commit_file_diff,
            cmds_history::cherry_pick,
            cmds_history::revert_commit,
            cmds_history::reset_to_commit,
            cmds_history::checkout_commit,
            // branches
            cmds_branches::list_branches,
            cmds_branches::create_branch,
            cmds_branches::checkout_branch,
            cmds_branches::delete_branch,
            cmds_branches::rename_branch,
            cmds_branches::merge_branch,
            cmds_branches::abort_merge,
            cmds_branches::rebase_branch,
            // remotes / network
            cmds_remotes::list_remotes,
            cmds_remotes::add_remote,
            cmds_remotes::remove_remote,
            cmds_remotes::rename_remote,
            cmds_remotes::set_remote_url,
            cmds_remotes::fetch_remote,
            cmds_remotes::fetch_all,
            cmds_remotes::pull,
            cmds_remotes::push,
            cmds_remotes::push_tag,
            cmds_remotes::set_branch_upstream,
            // stash & tags
            cmds_stash_tags::list_stashes,
            cmds_stash_tags::stash_save,
            cmds_stash_tags::stash_apply,
            cmds_stash_tags::stash_pop,
            cmds_stash_tags::stash_drop,
            cmds_stash_tags::list_tags,
            cmds_stash_tags::create_tag,
            cmds_stash_tags::delete_tag,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Git Crackit");
}
