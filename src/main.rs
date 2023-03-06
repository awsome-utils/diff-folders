use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use diff_folders::app::App;
use flexi_logger::writers::LogWriter;
use scopeguard::defer;
use std::{
    env::args,
    fs::{self, File, OpenOptions},
    io::{self, Error, ErrorKind, Write},
    path::{self},
    sync::{Arc, Mutex},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

fn main() -> Result<()> {
    if args().len() != 3 {
        panic!(
            "{} <old_dir|new_file> <new_dir|new_file>",
            args().next().unwrap()
        )
    }
    let (old_dir, new_dir) = parse_args();

    init_logger()?;
    setup_terminal()?;

    defer! {
        shutdown_terminal();
    }
    let mut terminal = start_terminal(io::stdout())?;

    let app = App::new(old_dir, new_dir);
    let res = run_app(&mut terminal, app);

    if let Err(err) = res {
        log::error!("{:?}", err)
    }
    Ok(())
}

fn parse_args() -> (String, String) {
    let mut args = args();
    args.next();
    let mut old_dir = args.next().unwrap();
    let mut new_dir = args.next().unwrap();
    if old_dir.ends_with(path::MAIN_SEPARATOR) {
        old_dir = old_dir[0..old_dir.len() - 1].to_string();
    }
    if new_dir.ends_with(path::MAIN_SEPARATOR) {
        new_dir = new_dir[0..new_dir.len() - 1].to_string();
    }
    (old_dir, new_dir)
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| app.draw(f))?;
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                _ => app.event(key.code),
            }
        }
    }
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    Ok(())
}

fn start_terminal<W: Write>(buf: W) -> io::Result<Terminal<CrosstermBackend<W>>> {
    let backend = CrosstermBackend::new(buf);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    terminal.clear()?;

    Ok(terminal)
}

fn shutdown_terminal() {
    let leave_screen = io::stdout().execute(LeaveAlternateScreen).map(|_f| ());

    if let Err(e) = leave_screen {
        log::error!("leave_screen failed:\n{e}\n");
    }

    let leave_raw_mode = disable_raw_mode();

    if let Err(e) = leave_raw_mode {
        log::error!("leave_raw_mode failed:\n{e}\n");
    }
}

fn init_logger() -> Result<()> {
    let dir = directories::BaseDirs::new()
        .unwrap()
        .home_dir()
        .join(".cache")
        .join("diff-folders");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let logfile = dir.clone().join("diff-folders.log");
    if !logfile.exists() {
        File::create(&logfile)?;
    }
    let fd = OpenOptions::new().write(true).append(true).open(logfile).unwrap();
    let my_writer = FileWriter {
        file: Arc::new(Mutex::new(fd)),
    };
    flexi_logger::Logger::try_with_str("info")
        .unwrap()
        .log_to_writer(Box::new(my_writer))
        .write_mode(flexi_logger::WriteMode::BufferAndFlush)
        .start()?;
    Ok(())
}

struct FileWriter<F> {
    file: Arc<Mutex<F>>,
}

impl<F: std::io::Write + Send + Sync> LogWriter for FileWriter<F> {
    fn write(
        &self,
        now: &mut flexi_logger::DeferredNow,
        record: &flexi_logger::Record,
    ) -> std::io::Result<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
        flexi_logger::detailed_format(&mut *file, now, record)
    }

    fn flush(&self) -> std::io::Result<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
        file.flush()
    }
}
