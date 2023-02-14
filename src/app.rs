use crate::status::{FolderStatefulList, StatefulList};
use crossterm::event::KeyCode;
use file_diff::diff;
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::convert::From;
use std::fs::File;
use std::io::{self, BufRead, Read};
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
    cur_file_path: Option<FolderStatefulList>,
    is_home: bool,
}

impl App {
    pub fn new(old_dir: String, new_dir: String) -> Self {
        let files = diff_list_dir(&old_dir, &new_dir);
        let items = StatefulList::with_items(files);
        Self {
            new_dir,
            old_dir,
            items,
            tab: WindowType::Left,
            scroll: 0,
            len_contents: 0,
            cur_file_path: None,
            is_home: false,
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
            KeyCode::Home => self.home(),
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
        self.is_home = false;
        self.cur_file_path = Some(self.items.cur().clone());
    }

    fn home(&mut self) {
        self.cur_file_path = Some(self.items.cur().clone());
        self.is_home = true;
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
                    crate::status::StatusItemType::Modified => {
                        Style::default().fg(Color::LightYellow)
                    }
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
            let (contents, title) = Self::get_diff_spans(file, &self.new_dir, &self.old_dir, self.is_home);
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
                .wrap(tui::widgets::Wrap { trim: false })
                .scroll((self.scroll, 0));
            f.render_widget(paragraph, chunks[1]);
        }
    }

    fn get_diff_spans<'a>(
        file: &FolderStatefulList,
        new_dir: &'a str,
        old_dir: &'a str,
        is_home: bool,
    ) -> (Vec<Spans<'a>>, String) {
        if is_home {
            return (
                vec![Spans::from(String::from_utf8(MSG.to_vec()).unwrap())],
                "letter".to_string(),
            );
        }
        if file.entry.path().is_dir() {
            return (
                vec![Spans::from("\n\nthis is directory")],
                "error".to_string(),
            );
        }
        let cur_file_path = match file.entry.path().to_str() {
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
            .expect(&format!("file not found: {}", cur_file_path))
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

        if file.state == crate::status::StatusItemType::Deleted
            || file.state == crate::status::StatusItemType::New
        {
            
            let buf = io::BufReader::new(buf_new.as_bytes());
            let contents: Vec<Spans> = buf
                .lines()
                .into_iter()
                .map(|i| Spans::from(Span::styled(i.unwrap(), Style::default().fg(Color::Red))))
                .collect();

            let mut title = format!("Deleted: {}", cur_file_path);
            if file.state == crate::status::StatusItemType::New {
                title = format!("New File: {}", cur_file_path);
            }
            return (contents, title);
        }

        let old_file_path = cur_file_path.replace(new_dir, old_dir);
        let mut buf_old = String::new();
        let err = File::open(&old_file_path)
            .expect(&format!("file not found: {}", old_file_path))
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

fn list_dir(path: &str) -> HashMap<String, DirEntry> {
    let mut files = HashMap::new();
    for f in walkdir::WalkDir::new(path) {
        let entry = f.unwrap();
        let key = entry
            .path()
            .to_str()
            .unwrap()
            .replace(path, &"".to_string());
        files.insert(key, entry);
    }
    files
}

fn diff_list_dir(old_dir: &str, new_dir: &str) -> Vec<FolderStatefulList> {
    let old_files = list_dir(old_dir);
    let new_files = list_dir(new_dir);
    let mut res = Vec::new();

    for (key, entry) in &old_files {
        match new_files.get(key) {
            None => {
                res.push(FolderStatefulList {
                    entry: entry.clone(),
                    state: crate::status::StatusItemType::Deleted,
                });
            }
            _ => {}
        }
    }

    for (key, entry) in &new_files {
        match old_files.get(key) {
            None => {
                res.push(FolderStatefulList {
                    entry: entry.clone(),
                    state: crate::status::StatusItemType::New,
                });
            }
            Some(_) => {
                if entry.path().is_file() {
                    let new_file_path = entry.path().to_str().unwrap();
                    let old_file_path = new_file_path.replace(new_dir, old_dir);
                    let err = File::open(&old_file_path);
                    match err {
                        Ok(_) => {
                            let is_same = diff(new_file_path, old_file_path.as_str());
                            if !is_same {
                                res.push(FolderStatefulList {
                                    entry: entry.clone(),
                                    state: crate::status::StatusItemType::Modified,
                                });
                            }
                            // * filter Normal
                            // else {
                            //     res.push(FolderStatefulList {
                            //         entry: entry.clone(),
                            //         state: crate::status::StatusItemType::Normal,
                            //     });
                            // }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    res.sort_by(|x, y| {
        x.entry
            .path()
            .to_str()
            .unwrap()
            .cmp(y.entry.path().to_str().unwrap())
    });
    res
}

const  MSG: [u8; 318] = [84, 104, 105, 115, 32, 112, 114, 111, 106, 101, 99, 116, 32, 119, 97, 115, 32, 105, 110, 115, 112, 105, 114, 101, 100, 32, 98, 121, 32, 109, 121, 32, 103, 105, 114, 108, 102, 114, 105, 101, 110, 100, 44, 32, 119, 104, 111, 32, 114, 101, 113, 117, 101, 115, 116, 101, 100, 32, 97, 32, 116, 111, 111, 108, 32, 102, 111, 114, 32, 99, 111, 109, 112, 97, 114, 105, 110, 103, 32, 100, 105, 114, 101, 99, 116, 111, 114, 105, 101, 115, 59, 32, 97, 108, 116, 104, 111, 117, 103, 104, 32, 116, 104, 111, 117, 103, 104, 32, 86, 83, 32, 67, 111, 100, 101, 32, 97, 108, 114, 101, 97, 100, 121, 32, 111, 102, 102, 101, 114, 115, 32, 115, 117, 99, 104, 32, 97, 32, 112, 108, 117, 103, 45, 105, 110, 44, 32, 73, 32, 115, 116, 105, 108, 108, 32, 119, 97, 110, 116, 32, 116, 111, 32, 99, 114, 101, 97, 116, 101, 32, 111, 110, 101, 32, 102, 111, 114, 32, 104, 101, 114, 32, 40, 109, 111, 115, 116, 108, 121, 32, 115, 105, 110, 99, 101, 32, 73, 32, 100, 111, 110, 39, 116, 32, 104, 97, 118, 101, 32, 97, 110, 121, 32, 109, 111, 110, 101, 121, 32, 116, 111, 32, 112, 117, 114, 99, 104, 97, 115, 101, 32, 111, 116, 104, 101, 114, 32, 116, 104, 105, 110, 103, 115, 41, 59, 10, 73, 32, 119, 105, 115, 104, 32, 102, 111, 114, 32, 101, 118, 101, 114, 121, 111, 110, 101, 39, 115, 32, 104, 97, 112, 112, 105, 110, 101, 115, 115, 44, 32, 104, 101, 97, 108, 116, 104, 44, 32, 97, 110, 100, 32, 105, 110, 99, 114, 101, 97, 115, 105, 110, 103, 32, 119, 101, 97, 108, 116, 104, 59, 10, 50, 48, 50, 51, 48, 50, 49, 52];