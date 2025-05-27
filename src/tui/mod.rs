// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
mod app;
mod event;
mod ratskin;
mod theme;
mod tool_info;
mod widgets;

pub use app::App;
pub use theme::Theme;
pub use widgets::*;

pub(crate) fn split_think_tags(text: &str) -> Vec<(String, bool)> {
    let mut result = vec![];
    let mut current_string = String::new();
    let mut tag_name = String::new();
    let mut is_tag_content = false;
    for ch in text.chars() {
        if ch == '<' {
            is_tag_content = true;
            if !tag_name.is_empty()
                && tag_name != "<think>"
                && tag_name != "<thinking>"
                && tag_name != "</think>"
                && tag_name != "</thinking>"
            {
                current_string.push_str(&tag_name);
            }
            tag_name.clear();
            tag_name.push(ch);
        } else if ch == '>' && is_tag_content {
            is_tag_content = false;
            tag_name.push(ch);
            // start think tag
            if (tag_name == "<think>" || tag_name == "<thinking>") && !current_string.is_empty() {
                result.push((current_string.clone(), false));
                current_string.clear();
            }
            if tag_name == "</think>" || tag_name == "</thinking>" && !current_string.is_empty() {
                result.push((current_string.clone(), true));
                current_string.clear();
            }
        } else if is_tag_content {
            tag_name.push(ch);
        } else {
            current_string.push(ch);
        }
    }
    if tag_name != "<think>"
        && tag_name != "<thinking>"
        && tag_name != "</think>"
        && tag_name != "</thinking>"
    {
        current_string.push_str(&tag_name);
        tag_name.clear();
    }

    if !current_string.is_empty() {
        result.push((
            current_string.clone(),
            tag_name == "<think>" || tag_name == "<thinking>",
        ));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_think_tags() {
        let text =
            "test <think>hello <- test </think> world -> test <- test <thinking>foo -> test</thinking> bar <think>baz";
        let pairs = split_think_tags(text);
        assert_eq!(
            pairs,
            vec![
                ("test ".to_string(), false),
                ("hello <- test ".to_string(), true),
                (" world -> test <- test ".to_string(), false),
                ("foo -> test".to_string(), true),
                (" bar ".to_string(), false),
                ("baz".to_string(), true)
            ]
        );
    }

    #[test]
    fn test_split_think_no_tags() {
        let text = "test message.";
        let pairs = split_think_tags(text);
        assert_eq!(pairs, vec![("test message.".to_string(), false),]);
    }
}
