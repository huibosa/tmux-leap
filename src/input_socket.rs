use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{BufRead, BufReader, Write};
use crate::config::socket_path;

pub struct InputSocket {
    path: std::path::PathBuf,
    listener: UnixListener,
}

impl InputSocket {
    pub fn new() -> std::io::Result<Self> {
        let path = socket_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        Ok(InputSocket { path, listener })
    }

    /// Block until a message arrives; returns the message string.
    pub fn recv(&self) -> std::io::Result<String> {
        let (stream, _) = self.listener.accept()?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        Ok(line.trim_end_matches('\n').to_string())
    }

    pub fn close(self) {
        drop(self.listener);
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Send a single-line message to the running leap socket and return.
pub fn send(message: &str) -> std::io::Result<()> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)?;
    writeln!(stream, "{}", message)?;
    Ok(())
}
