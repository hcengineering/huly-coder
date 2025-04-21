pub mod access_mcp_resource;
pub mod ask_followup_question;
pub mod attempt_completion;
pub mod execute_command;
pub mod list_code_definition_names;
pub mod list_files;
pub mod read_file;
pub mod replace_in_file;
pub mod search_files;
pub mod use_mcp_tool;
pub mod write_to_file;

pub fn create_patch(original: &str, modified: &str) -> String {
    diffy::create_patch(original, modified)
        .to_string()
        .lines()
        .skip(2)
        .collect::<String>()
}
