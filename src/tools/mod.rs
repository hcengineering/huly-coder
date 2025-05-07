use std::path::Path;

pub mod ask_followup_question;
pub mod attempt_completion;
pub mod execute_command;
pub mod list_files;
pub mod read_file;
pub mod replace_in_file;
pub mod search_files;
pub mod write_to_file;

pub fn create_patch(original: &str, modified: &str) -> String {
    diffy::create_patch(original, modified)
        .to_string()
        .lines()
        .skip(2)
        .collect::<String>()
}

#[inline]
pub fn workspace_to_string(workspace: &Path) -> String {
    workspace.to_str().unwrap().to_string().replace("\\", "/")
}

pub fn normalize_path(workspace: &Path, path: &str) -> String {
    let path = path.to_string().replace("\\", "/");
    let workspace = workspace_to_string(workspace);
    if !path.starts_with(&workspace) {
        format!("{}/{}", workspace, path)
    } else {
        path
    }
}
