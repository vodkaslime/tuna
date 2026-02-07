use std::io::{Read, Write};
use std::thread;

use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("failed to spawn pty")]
    Spawn(#[source] anyhow::Error),
    #[error("failed to resize pty")]
    Resize(#[source] anyhow::Error),
    #[error("pty io error")]
    Io(#[source] std::io::Error),
}

#[derive(Debug, Clone, Copy)]
pub struct PtyDimensions {
    pub cols: u16,
    pub rows: u16,
}

impl PtyDimensions {
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }
}

#[derive(Debug)]
pub enum PtyEvent {
    Output(Vec<u8>),
    Exited(i32),
}

pub struct PtySession {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
    child: Mutex<Box<dyn portable_pty::Child + Send>>,
}

impl PtySession {
    pub fn spawn_shell(dimensions: PtyDimensions) -> Result<(Self, Receiver<PtyEvent>), PtyError> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: dimensions.rows,
                cols: dimensions.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(PtyError::Spawn)?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = CommandBuilder::new(shell);
        // Ensure programs have a sensible terminal type; many TUI frameworks assume this exists.
        cmd.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(PtyError::Spawn)?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(PtyError::Spawn)?;

        let writer = pair
            .master
            .take_writer()
            .map_err(PtyError::Spawn)?;

        let master: Box<dyn portable_pty::MasterPty + Send> = pair.master;

        let (tx, rx) = crossbeam_channel::unbounded();
        spawn_reader_thread(reader, tx);

        Ok((
            Self {
                writer: Mutex::new(writer),
                master: Mutex::new(master),
                child: Mutex::new(child),
            },
            rx,
        ))
    }

    pub fn write_all(&self, bytes: &[u8]) -> Result<(), PtyError> {
        let mut writer = self.writer.lock();
        writer.write_all(bytes).map_err(PtyError::Io)?;
        writer.flush().map_err(PtyError::Io)?;
        Ok(())
    }

    pub fn resize(&self, dimensions: PtyDimensions) -> Result<(), PtyError> {
        self.master
            .lock()
            .resize(PtySize {
                rows: dimensions.rows,
                cols: dimensions.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(PtyError::Resize)?;
        Ok(())
    }

    pub fn kill(&self) -> Result<(), PtyError> {
        self.child
            .lock()
            .kill()
            .map_err(PtyError::Io)?;
        Ok(())
    }
}

fn spawn_reader_thread(mut reader: Box<dyn Read + Send>, tx: Sender<PtyEvent>) {
    // NOTE: portable-pty's reader doesn't expose non-blocking configuration in a cross-platform way.
    // We keep a blocking read loop on a dedicated thread and forward bytes to the app via a channel.
    thread::spawn(move || {
        let mut buf = [0_u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(PtyEvent::Output(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    tracing::warn!(?err, "pty read error");
                    break;
                }
            }
        }

        let _ = tx.send(PtyEvent::Exited(-1));
    });
}
