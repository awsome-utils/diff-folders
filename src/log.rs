use anyhow::Result;
use flexi_logger::writers::LogWriter;
use std::{
    fs::{self, File, OpenOptions},
    io::{Error, ErrorKind},
    sync::{Arc, Mutex},
};

pub fn init_logger() -> Result<()> {
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
    let fd = OpenOptions::new()
        .write(true)
        .append(true)
        .open(logfile)
        .unwrap();
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
