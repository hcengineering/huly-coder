// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::agent::utils::MAX_FILES;
use crate::tui::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::{
    Block, BorderType, Borders, Scrollbar, ScrollbarOrientation, StatefulWidget,
};
use tui_tree_widget::{Tree, TreeItem, TreeState};

#[derive(Debug)]
pub struct FileTreeState {
    pub workspace: PathBuf,
    pub items: Vec<TreeItem<'static, String>>,
    pub tree_state: TreeState<String>,
    pub focused: bool,
    pub highlighted: bool,
}

#[derive(Debug)]
struct FileDirTreeItem {
    pub path: String,
    pub name: String,
    pub children: Vec<Rc<RefCell<FileDirTreeItem>>>,
}

impl FileDirTreeItem {
    pub fn into(item: Rc<RefCell<Self>>) -> TreeItem<'static, String> {
        TreeItem::new(
            item.as_ref().borrow().path.clone(),
            item.as_ref().borrow().name.clone(),
            item.as_ref()
                .borrow()
                .children
                .clone()
                .into_iter()
                .map(|child| FileDirTreeItem::into(child))
                .collect(),
        )
        .unwrap()
    }
}

impl FileTreeState {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            items: Vec::default(),
            tree_state: TreeState::default(),
            focused: false,
            highlighted: false,
        }
    }

    pub fn highlight_file(&mut self, path: String) {
        let path = path.replace("\\", "/");
        let path = path.trim_start_matches("./");
        tracing::debug!("highlight_file: {}", path);
        self.tree_state.close_all();
        let mut opened = Vec::new();
        let mut dir = String::new();
        for part in path.split('/') {
            if dir.is_empty() {
                dir = part.to_string();
            } else {
                dir = format!("{}/{}", dir, part);
            }
            opened.push(dir.clone());
            self.tree_state.open(opened.clone());
        }
        self.tree_state.select(opened);
        self.tree_state.scroll_selected_into_view();
        self.highlighted = true;
    }

    pub fn update_items(&mut self) {
        self.items.clear();
        let mut roots: HashMap<String, Rc<RefCell<FileDirTreeItem>>> = HashMap::new();
        let mut files = vec![];
        ignore::WalkBuilder::new(&self.workspace)
            .filter_entry(|e| e.file_name() != "node_modules")
            .build()
            .filter_map(|e| e.ok())
            .take(MAX_FILES)
            .for_each(|entry| {
                let path = entry.path().strip_prefix(&self.workspace).unwrap();
                let metadata = entry.metadata().unwrap();
                if let Some(file_name) = path.file_name() {
                    let file_name = file_name.to_string_lossy().to_string();
                    let file_path = path.to_string_lossy().to_string().replace("\\", "/");
                    let parent_path = path
                        .parent()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                        .replace("\\", "/");
                    let tree_item = Rc::new(RefCell::new(FileDirTreeItem {
                        path: file_path.clone(),
                        name: file_name.clone(),
                        children: vec![],
                    }));
                    if metadata.is_file() && path.components().count() == 1 {
                        // root files
                        files.push(tree_item);
                    } else {
                        if metadata.is_dir() {
                            roots.insert(file_path, Rc::clone(&tree_item));
                        }
                        if let Some(parent) = roots.get_mut(&parent_path) {
                            let mut parent = RefCell::borrow_mut(parent);
                            parent.children.push(tree_item);
                        } else {
                            files.push(tree_item);
                        }
                    }
                }
            });
        self.items = files
            .into_iter()
            .map(|item| FileDirTreeItem::into(item))
            .collect();
    }
}

#[derive(Debug)]
pub struct FileTreeWidget;

impl FileTreeWidget {
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut FileTreeState, theme: &Theme) {
        if state.items.is_empty() {
            state.update_items();
        }
        let file_tree_block = Block::bordered()
            .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
            .title(" Workspace ")
            .title_alignment(Alignment::Right)
            .title_style(theme.text_style())
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(state.focused));

        let widget = Tree::new(&state.items)
            .expect("all item identifiers are unique")
            .block(file_tree_block)
            .experimental_scrollbar(Some(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .track_symbol(None)
                    .end_symbol(None)
                    .thumb_symbol("▐"),
            ))
            .highlight_style(if state.highlighted {
                Style::new().bg(theme.focus)
            } else {
                Style::new().bg(theme.border)
            });
        StatefulWidget::render(widget, area, buf, &mut state.tree_state);
    }
}
