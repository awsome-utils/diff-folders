use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use diff_folders::{app::App, log::init_logger};
use scopeguard::defer;
use std::{
    env::args,
    io::{self, Write},
    path::{self, Path},
    str::FromStr,
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
    let new_dir = String::from_str(
        Path::new(&new_dir)
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let old_dir = String::from_str(
        Path::new(&old_dir)
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap();
    (old_dir, new_dir)
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        app.draw_terminal(terminal)?;
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
