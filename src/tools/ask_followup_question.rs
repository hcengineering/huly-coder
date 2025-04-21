use std::collections::HashMap;

use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskFollowupQuestionToolArgs {
    pub question: String,
    pub options: String,
}

pub struct AskFollowupQuestionTool;

impl Tool for AskFollowupQuestionTool {
    const NAME: &'static str = "ask_followup_question";

    type Error = std::io::Error;
    type Args = AskFollowupQuestionToolArgs;
    type Output = String;

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
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
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

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok("".to_string())
    }
}
