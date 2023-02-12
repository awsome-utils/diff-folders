use crate::status::{FolderStatefulList, StatefulList};
use crossterm::event::KeyCode;
use file_diff::diff;
use similar::{ChangeTag, TextDiff};
use std::convert::From;
use std::fs::File;
use std::io::Read;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, List, ListItem, Paragraph};
use tui::{backend::Backend, Frame};
use walkdir::DirEntry;

enum WindowType {
    Left,
    Right,
}

pub struct App {
    new_dir: String,
    old_dir: String,
    items: StatefulList<FolderStatefulList>,
    tab: WindowType,

    // window status
    scroll: u16,
    len_contents: usize,
    cur_file_path: Option<DirEntry>,
}

impl App {
    pub fn new(old_dir: String, new_dir: String) -> Self {
        let files = diff_dir(&old_dir, &new_dir);
        let items = StatefulList::with_items(files);
        Self {
            new_dir,
            old_dir,
            items,
            tab: WindowType::Left,
            scroll: 0,
            len_contents: 0,
            cur_file_path: None,
        }
    }

    pub fn event(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Left => {
                self.left();
            }
            KeyCode::Right => {
                self.right();
            }
            KeyCode::Down => {
                self.down();
            }
            KeyCode::Up => {
                self.up();
            }
            KeyCode::Enter => self.enter(),
            _ => {}
        }
    }

    fn left(&mut self) {
        match self.tab {
            WindowType::Right => self.tab = WindowType::Left,
            _ => {}
        }
    }

    fn right(&mut self) {
        match self.tab {
            WindowType::Left => self.tab = WindowType::Right,
            _ => {}
        }
    }

    fn up(&mut self) {
        match self.tab {
            WindowType::Left => self.items.previous(),
            WindowType::Right => {
                if self.scroll > 0 {
                    self.scroll -= 1
                }
            }
        }
    }

    fn down(&mut self) {
        match self.tab {
            WindowType::Left => self.items.next(),
            WindowType::Right => {
                let total = self.len_contents as u16;
                if self.scroll >= total {
                    self.scroll = total
                } else {
                    self.scroll += 1
                }
            }
        }
    }

    fn enter(&mut self) {
        self.cur_file_path = Some(self.items.cur().entry.clone());
    }

    pub fn draw<B: Backend>(&mut self, f: &mut Frame<B>) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints(
                match self.tab {
                    WindowType::Left => [Constraint::Percentage(90), Constraint::Percentage(10)],
                    WindowType::Right => [Constraint::Percentage(10), Constraint::Percentage(90)],
                }
                .as_ref(),
            )
            .split(f.size());
        let items: Vec<ListItem> = self
            .items
            .items
            .iter()
            .map(|i| {
                let path = match i.entry.path().to_str() {
                    Some(p) => {
                        if i.entry.path().is_dir() {
                            format!("d {}", p)
                        } else {
                            format!("f {}", p)
                        }
                    }
                    None => "".to_owned(),
                };
                let lines = vec![Spans::from(path)];
                ListItem::new(lines).style(match i.state {
                    crate::status::StatusItemType::Deleted => Style::default().fg(Color::Red),
                    crate::status::StatusItemType::Modified => Style::default().fg(Color::Red),
                    crate::status::StatusItemType::New => Style::default().fg(Color::Green),
                    crate::status::StatusItemType::Normal => Style::default(),
                })
            })
            .collect();
        let items = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(match self.tab {
                        WindowType::Left => Style::default().fg(Color::Blue),
                        WindowType::Right => Style::default(),
                    })
                    .title(format!("folder {}", self.new_dir)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::ITALIC),
            );
        f.render_stateful_widget(items, chunks[0], &mut self.items.state);

        if let Some(file) = &self.cur_file_path {
            let (contents, title) = Self::get_diff_spans(file, &self.new_dir, &self.old_dir);
            self.len_contents = contents.len() as usize;
            let paragraph = Paragraph::new(contents)
                .style(Style::default())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(match self.tab {
                            WindowType::Left => Style::default(),
                            WindowType::Right => Style::default().fg(Color::Blue),
                        })
                        .title(title),
                )
                .scroll((self.scroll, 0));
            f.render_widget(paragraph, chunks[1]);
        }
    }

    fn get_diff_spans<'a>(file:&DirEntry, new_dir: &'a str, old_dir: &'a str) -> (Vec<Spans<'a>>, String) {
        if file.path().is_dir() {
            return (vec![Spans::from("this is directory")], "error".to_string());
        }
        let cur_file_path = match file.path().to_str() {
            Some(p) => p,
            None => "",
        };
        if cur_file_path == "" {
            return (
                vec![Spans::from("please press 'enter', select file")],
                "error".to_string(),
            );
        }
        let mut buf_new = String::new();
        let err = File::open(cur_file_path)
            .expect("file not found")
            .read_to_string(&mut buf_new);
        if err.is_err() {
            return (
                vec![Spans::from(format!(
                    "open file:{}, error: {}",
                    cur_file_path,
                    err.err().unwrap()
                ))],
                "error".to_string(),
            );
        }

        let old_file_path = cur_file_path.replace(new_dir, old_dir);
        let mut buf_old = String::new();
        let err = File::open(&old_file_path)
            .expect("file not found")
            .read_to_string(&mut buf_old);
        if err.is_err() {
            return (
                vec![Spans::from(format!(
                    "open file:{}, error: {}",
                    old_file_path,
                    err.err().unwrap()
                ))],
                "error".to_string(),
            );
        }

        let diff = TextDiff::from_lines(&buf_old, &buf_new);
        let contents: Vec<Spans> = diff
            .iter_all_changes()
            .into_iter()
            .map(|i| {
                let (sign, color) = match i.tag() {
                    ChangeTag::Delete => ("-", Color::Red),
                    ChangeTag::Insert => ("+", Color::Green),
                    ChangeTag::Equal => (" ", Color::White),
                };
                Spans::from(Span::styled(
                    format!("{} {}", sign, i),
                    Style::default().fg(color),
                ))
            })
            .collect();
        let title = format!("Diff: {} and {}", cur_file_path, old_file_path);
        (contents, title)
    }
}

fn diff_dir(old_path: &str, new_path: &str) -> Vec<FolderStatefulList> {
    let mut files = Vec::new();
    for f in walkdir::WalkDir::new(new_path) {
        let f = f.unwrap();
        let mut fs = FolderStatefulList {
            entry: f,
            state: crate::status::StatusItemType::Normal,
        };
        match fs.entry.path().to_str() {
            Some(fpath) => {
                if fs.entry.path().is_file() {
                    let old_file_path = fpath.replace(new_path, old_path);
                    let err = File::open(&old_file_path);
                    match err {
                        Ok(_) => {
                            let is_same = diff(fpath, old_file_path.as_str());
                            if !is_same {
                                fs.state = crate::status::StatusItemType::Modified
                            }
                        }
                        _ => {
                            fs.state = crate::status::StatusItemType::New;
                        }
                    }
                }
            }
            None => {}
        }
        if fs.state != crate::status::StatusItemType::Normal {
            files.push(fs);
        }
    }
    files
}
