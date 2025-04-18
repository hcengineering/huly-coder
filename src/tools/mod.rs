use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;

use rig::completion::ToolDefinition;
use rig::tool::ToolError;

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

pub trait ClineTool: Sized + Send + Sync {
    const NAME: &'static str;

    type Error: std::error::Error + Send + Sync + 'static;

    fn name(&self) -> String {
        Self::NAME.to_string()
    }

    fn definition(&self, _prompt: String) -> impl Future<Output = ToolDefinition> + Send + Sync;

    fn call(
        &self,
        args: &HashMap<String, String>,
    ) -> impl Future<Output = Result<String, Self::Error>> + Send + Sync;

    fn usage(&self) -> &str;
}

pub trait ClineToolDyn: Send + Sync {
    fn name(&self) -> String;

    fn definition(
        &self,
        prompt: String,
    ) -> Pin<Box<dyn Future<Output = ToolDefinition> + Send + Sync + '_>>;

    fn call(
        &self,
        args: HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + Sync + '_>>;

    fn usage(&self) -> &str;
}

impl<T: ClineTool> ClineToolDyn for T {
    fn name(&self) -> String {
        self.name()
    }

    fn definition(
        &self,
        prompt: String,
    ) -> Pin<Box<dyn Future<Output = ToolDefinition> + Send + Sync + '_>> {
        Box::pin(<Self as ClineTool>::definition(self, prompt))
    }

    fn call(
        &self,
        args: HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + Sync + '_>> {
        Box::pin(async move {
            <Self as ClineTool>::call(self, &args).await.map_err(|e| {
                let boxed_error: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
                ToolError::ToolCallError(boxed_error)
            })
        })
    }

    fn usage(&self) -> &str {
        self.usage()
    }
}

#[derive(Debug, PartialEq)]
enum State {
    LookingForToolStart,
    ReadingToolName,
    LookingForParamStart,
    ReadingParamName,
    ReadingParamValue,
}

pub struct ToolCallIterator {
    idx: usize,
    text: Vec<char>,
    state: State,
    tool_names: HashSet<String>,
}

impl ToolCallIterator {
    pub fn new(text: &str, tool_names: HashSet<String>) -> Self {
        Self {
            idx: 0,
            text: text.chars().collect(),
            state: State::LookingForToolStart,
            tool_names,
        }
    }
}

/// Returns an iterator over the tool calls in the given string.
impl Iterator for ToolCallIterator {
    type Item = (String, HashMap<String, String>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.text.len() {
            return None;
        }

        let mut tool_name = String::new();
        let mut param_name = String::new();
        let mut param_value: String;
        let mut params = HashMap::new();
        let mut tool_start_idx = 0;
        let mut param_start_idx = 0;
        let mut param_value_start_idx = 0;

        while self.idx < self.text.len() {
            let c = self.text[self.idx];

            match self.state {
                State::LookingForToolStart => {
                    if c == '<' {
                        tool_start_idx = self.idx + 1;
                        self.state = State::ReadingToolName;
                    }
                }

                State::ReadingToolName => {
                    if c == '>' {
                        tool_name = self.text[tool_start_idx..self.idx].iter().collect();
                        if self.tool_names.contains(&tool_name) {
                            self.state = State::LookingForParamStart;
                        } else {
                            self.state = State::LookingForToolStart;
                            tool_name.clear();
                        }
                    }
                }

                State::LookingForParamStart => {
                    if c == '<' && self.idx + 1 < self.text.len() {
                        let next_char = self.text[self.idx + 1];
                        // check for closing tag of the root tool element
                        if next_char == '/' {
                            let expected_close_tag = format!("</{}>", tool_name);
                            if self.idx + expected_close_tag.len() <= self.text.len() {
                                let close_tag_name: String = self.text
                                    [self.idx..(self.idx + expected_close_tag.len())]
                                    .iter()
                                    .collect();
                                if close_tag_name == expected_close_tag {
                                    self.idx += expected_close_tag.len() - 1;
                                    self.state = State::LookingForToolStart;
                                    return Some((tool_name, params));
                                }
                            }
                        } else {
                            param_start_idx = self.idx + 1;
                            self.state = State::ReadingParamName;
                        }
                    }
                }

                State::ReadingParamName => {
                    if c == '>' {
                        param_name = self.text[param_start_idx..self.idx].iter().collect();
                        param_value_start_idx = self.idx + 1;
                        self.state = State::ReadingParamValue;
                    }
                }

                State::ReadingParamValue => {
                    if c == '<' {
                        param_value = self.text[param_value_start_idx..self.idx].iter().collect();

                        let expected_close_tag = format!("</{}>", param_name);
                        if self.idx + expected_close_tag.len() <= self.text.len() {
                            let close_tag_name: String = self.text
                                [self.idx..(self.idx + expected_close_tag.len())]
                                .iter()
                                .collect();
                            if close_tag_name == expected_close_tag {
                                params.insert(param_name.clone(), param_value.clone());

                                param_name.clear();
                                param_value.clear();

                                self.idx += expected_close_tag.len() - 1;
                                self.state = State::LookingForParamStart;
                            }
                        }
                    }
                }
            }
            self.idx += 1;
        }

        if !tool_name.is_empty() && !params.is_empty() {
            Some((tool_name, params))
        } else {
            None
        }
    }
}

pub fn create_patch(original: &str, modified: &str) -> String {
    diffy::create_patch(original, modified)
        .to_string()
        .lines()
        .skip(2)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call_iterator() {
        let txt = "some test message:\n\n<thinking>\n1. some thinking block.\n</thinking>\n\n<read_file>\n<path>cline-agent.d</path>\n\n</read_file>\n<thinking>\nanother thinking block.\n</thinking>";
        let tool_names = HashSet::from(["read_file".to_string()]);
        let mut iterator = ToolCallIterator::new(txt, tool_names);
        assert_eq!(
            Some((
                "read_file".to_string(),
                HashMap::from([("path".to_string(), "cline-agent.d".to_string())])
            )),
            iterator.next()
        );
        assert_eq!(None, iterator.next())
    }

    #[test]
    fn test_tool_call_iterator_complex() {
        let txt = "some test message:\n\n<thinking>\n1. some thinking block.\n</thinking>\n\n<read_file>\n<path>cline-agent.d</path>\n\n</read_file>\n just text block \n\n<write_to_file>\n<path>index.html</path>\n<content><!DOCTYPE html>\n<html lang=\"en\">\n<head>\n    <meta charset=\"UTF-8\">\n\n</body>\n</html>\n</content>\n</write_to_file>\n<thinking>\nanother thinking block.\n</thinking>";
        let tool_names = HashSet::from(["read_file".to_string(), "write_to_file".to_string()]);
        let mut iterator = ToolCallIterator::new(txt, tool_names);
        assert_eq!(
            Some((
                "read_file".to_string(),
                HashMap::from([("path".to_string(), "cline-agent.d".to_string())])
            )),
            iterator.next()
        );
        assert_eq!(
            Some((
                "write_to_file".to_string(),
                HashMap::from([
                    ("path".to_string(), "index.html".to_string()),
                    ("content".to_string(), "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n    <meta charset=\"UTF-8\">\n\n</body>\n</html>\n".to_string())
                ])
            )),
            iterator.next()
        );
        assert_eq!(None, iterator.next())
    }
}
