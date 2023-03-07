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
use tui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};
use tui::Terminal;
use tui::{backend::Backend, Frame};
use walkdir::DirEntry;

enum WindowType {
    Left,
    Right,
}
pub struct App {
    new_dir: String,
    old_dir: String,
    tab: WindowType,
    items: StatefulList<FolderStatefulList>,

    // window status
    scroll: u16,
    len_contents: usize,
    cur_file_path: Option<FolderStatefulList>,

    page_size: u16,
    is_home: bool,
    is_loaded: bool,
}

impl App {
    pub fn new(old_dir: String, new_dir: String) -> Self {
        Self {
            new_dir,
            old_dir,
            tab: WindowType::Left,
            scroll: 0,
            len_contents: 0,
            cur_file_path: None,
            is_home: false,
            is_loaded: false,
            page_size: 0,
            items: StatefulList::with_items(Vec::new()),
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
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
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
            WindowType::Left => {
                self.items.previous(1);
                self.enter();
            }
            WindowType::Right => {
                if self.scroll > 0 {
                    self.scroll -= 1
                }
            }
        }
    }

    fn down(&mut self) {
        match self.tab {
            WindowType::Left => {
                self.items.next(1);
                self.enter();
            }
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
        if let Some(file) = &self.cur_file_path {
            if file.entry.path() == self.items.cur().entry.path() {
                // same file
                return;
            }
        }
        self.cur_file_path = Some(self.items.cur().clone());
        self.scroll = 0
    }

    fn home(&mut self) {
        self.cur_file_path = Some(self.items.cur().clone());
        self.is_home = true;
    }

    fn page_up(&mut self) {
        match self.tab {
            WindowType::Left => {
                self.items.previous(self.page_size as usize);
                self.enter();
            }
            WindowType::Right => {
                let mut page_size = self.page_size;
                let content_length = self.len_contents as u16;
                if page_size > content_length {
                    page_size = content_length;
                }
                if self.scroll < page_size {
                    self.scroll = 0
                } else {
                    self.scroll -= page_size
                }
            }
        }
    }

    fn page_down(&mut self) {
        match self.tab {
            WindowType::Left => {
                self.items.next(self.page_size as usize);
                self.enter();
            }
            WindowType::Right => {
                let mut page_size = self.page_size;
                let content_length = self.len_contents as u16;
                if page_size > content_length {
                    page_size = content_length;
                }
                if self.scroll + page_size >= content_length {
                    self.scroll = content_length
                } else {
                    self.scroll += page_size
                }
            }
        }
    }

    fn draw_gauge<B: Backend>(&mut self, terminal: &mut Terminal<B>) {
        self.diff_list_dir(&mut move |p| {
            let _ = terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints(
                        [
                            Constraint::Percentage(40),
                            Constraint::Length(5),
                            Constraint::Percentage(40),
                        ]
                        .as_ref(),
                    )
                    .split(f.size());
                let gauge = Gauge::default()
                    .block(
                        Block::default()
                            .title("Loading files")
                            .borders(Borders::ALL),
                    )
                    .gauge_style(Style::default().fg(Color::White))
                    .percent(p);
                f.render_widget(gauge, chunks[1]);
            }); // loading files
        });
    }

    pub fn draw_terminal<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        if !self.is_loaded {
            self.draw_gauge(terminal);
            self.is_loaded = true;
        }
        terminal.draw(|f| self.draw(f))?;
        return Ok(());
    }

    pub fn draw<B: Backend>(&mut self, f: &mut Frame<B>) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints(
                match self.tab {
                    WindowType::Left => [Constraint::Percentage(70), Constraint::Percentage(30)],
                    WindowType::Right => [Constraint::Percentage(30), Constraint::Percentage(70)],
                }
                .as_ref(),
            )
            .split(f.size());

        self.page_size = chunks[0].height / 2;

        let items: Vec<ListItem> = self
            .items
            .items
            .iter()
            .map(|i| {
                let path = match i.entry.path().to_str() {
                    Some(p) => {
                        let cur_path = p.replace(&self.new_dir, ".");
                        if i.entry.path().is_dir() {
                            format!("d {}", cur_path)
                        } else {
                            format!("f {}", cur_path)
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
                        WindowType::Left => Style::default().fg(Color::Gray),
                        WindowType::Right => Style::default().fg(Color::Black),
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
            let (contents, title) =
                Self::get_diff_spans(file, &self.new_dir, &self.old_dir, self.is_home);
            self.len_contents = contents.len() as usize;
            let paragraph = Paragraph::new(contents)
                .style(Style::default())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(match self.tab {
                            WindowType::Left => Style::default().fg(Color::Black),
                            WindowType::Right => Style::default().fg(Color::Gray),
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
            let mut title = format!("Deleted: {}", cur_file_path);
            let mut style = Color::Red;
            if file.state == crate::status::StatusItemType::New {
                title = format!("New File: {}", cur_file_path);
                style = Color::Green;
            }
            let buf = io::BufReader::new(buf_new.as_bytes());
            let contents: Vec<Spans> = buf
                .lines()
                .into_iter()
                .map(|i| Spans::from(Span::styled(i.unwrap(), Style::default().fg(style))))
                .collect();
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

    fn diff_list_dir(&mut self, progress: &mut impl FnMut(u16)) {
        progress(10);
        let old_dir = &self.old_dir;
        let new_dir = &self.new_dir;
        let old_files = list_dir(old_dir);
        progress(20);
        let new_files = list_dir(new_dir);
        progress(30);
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
        progress(40);

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
                        let new_file_path = entry.path().canonicalize().unwrap();
                        let old_file_path =
                            new_file_path.to_str().unwrap().replace(new_dir, old_dir);
                        let err = File::open(&old_file_path);
                        match err {
                            Ok(_) => {
                                let is_same =
                                    diff(new_file_path.to_str().unwrap(), old_file_path.as_str());
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
        progress(80);
        delta_folder_stateful_list(&mut res);
        self.items = StatefulList::with_items(res);
        progress(100);
    }
}

fn list_dir(path: &str) -> HashMap<String, DirEntry> {
    let mut files = HashMap::new();
    for f in walkdir::WalkDir::new(path) {
        let entry = f.unwrap();
        let key = entry
            .path()
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .replace(path, &"".to_string());
        files.insert(key, entry);
    }
    files
}

fn delta_folder_stateful_list(files: &mut Vec<FolderStatefulList>) {
    files.sort_by(|x, y| {
        x.entry
            .path()
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .cmp(y.entry.path().canonicalize().unwrap().to_str().unwrap())
    });
    let mut i = 1;
    while i < files.len() - 1 {
        // same directory
        if files[i - 1].entry.path().is_dir()
            && (files[i - 1].state == crate::status::StatusItemType::Deleted
                || files[i - 1].state == crate::status::StatusItemType::New)
        {
            if files[i]
                .entry
                .path()
                .to_str()
                .unwrap()
                .starts_with(files[i - 1].entry.path().to_str().unwrap())
            {
                files.remove(i);
                continue;
            }
        }
        i += 1;
    }
}

const MSG: [u8; 318] = [
    84, 104, 105, 115, 32, 112, 114, 111, 106, 101, 99, 116, 32, 119, 97, 115, 32, 105, 110, 115,
    112, 105, 114, 101, 100, 32, 98, 121, 32, 109, 121, 32, 103, 105, 114, 108, 102, 114, 105, 101,
    110, 100, 44, 32, 119, 104, 111, 32, 114, 101, 113, 117, 101, 115, 116, 101, 100, 32, 97, 32,
    116, 111, 111, 108, 32, 102, 111, 114, 32, 99, 111, 109, 112, 97, 114, 105, 110, 103, 32, 100,
    105, 114, 101, 99, 116, 111, 114, 105, 101, 115, 59, 32, 97, 108, 116, 104, 111, 117, 103, 104,
    32, 116, 104, 111, 117, 103, 104, 32, 86, 83, 32, 67, 111, 100, 101, 32, 97, 108, 114, 101, 97,
    100, 121, 32, 111, 102, 102, 101, 114, 115, 32, 115, 117, 99, 104, 32, 97, 32, 112, 108, 117,
    103, 45, 105, 110, 44, 32, 73, 32, 115, 116, 105, 108, 108, 32, 119, 97, 110, 116, 32, 116,
    111, 32, 99, 114, 101, 97, 116, 101, 32, 111, 110, 101, 32, 102, 111, 114, 32, 104, 101, 114,
    32, 40, 109, 111, 115, 116, 108, 121, 32, 115, 105, 110, 99, 101, 32, 73, 32, 100, 111, 110,
    39, 116, 32, 104, 97, 118, 101, 32, 97, 110, 121, 32, 109, 111, 110, 101, 121, 32, 116, 111,
    32, 112, 117, 114, 99, 104, 97, 115, 101, 32, 111, 116, 104, 101, 114, 32, 116, 104, 105, 110,
    103, 115, 41, 59, 10, 73, 32, 119, 105, 115, 104, 32, 102, 111, 114, 32, 101, 118, 101, 114,
    121, 111, 110, 101, 39, 115, 32, 104, 97, 112, 112, 105, 110, 101, 115, 115, 44, 32, 104, 101,
    97, 108, 116, 104, 44, 32, 97, 110, 100, 32, 105, 110, 99, 114, 101, 97, 115, 105, 110, 103,
    32, 119, 101, 97, 108, 116, 104, 59, 10, 50, 48, 50, 51, 48, 50, 49, 52,
];
