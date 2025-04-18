use std::collections::HashMap;

use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum AskFollowupQuestionError {
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

pub struct AskFollowupQuestionTool;

impl ClineTool for AskFollowupQuestionTool {
    const NAME: &'static str = "ask_followup_question";

    type Error = AskFollowupQuestionError;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Ask the user a question to gather additional information needed to complete the task. This tool should \
                be used when you encounter ambiguities, need clarification, or require more details to proceed effectively. \
                It allows for interactive problem-solving by enabling direct communication with the user. Use this tool \
                judiciously to maintain a balance between gathering necessary information and avoiding excessive back-and-forth."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to ask the user. This should be a clear, specific question that addresses the information you need.",
                    },
                    "options": {
                        "type": "string",
                        "description": indoc! {"\
                            An array of 2-5 options for the user to choose from. Each option should be a string describing \
                            a possible answer. You may not always need to provide options, but it may be helpful in many \
                            cases where it can save the user from having to type out a response manually. \
                            IMPORTANT: NEVER include an option to toggle to Act mode, as this would be something you need \
                            to direct the user to do manually themselves if needed."}
                    },
                },
                "required": ["question"]
            })

        }
    }

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        if let Some(_question) = args.get("question") {
            Ok("".to_string())
        } else {
            Err(AskFollowupQuestionError::ParametersError(
                "question".to_string(),
            ))
        }
    }

    fn usage(&self) -> &str {
        indoc! {r#"
            <ask_followup_question>
            <question>Your question here</question>
            <options>
            Array of options here (optional), e.g. ["Option 1", "Option 2", "Option 3"]
            </options>
            </ask_followup_question>
        "#}
    }
}
