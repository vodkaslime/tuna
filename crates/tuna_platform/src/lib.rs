use std::io;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn set_clipboard_text(text: &str) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        return set_clipboard_text_linux(text);
    }

    #[cfg(target_os = "macos")]
    {
        return set_clipboard_text_macos(text);
    }

    #[cfg(target_os = "windows")]
    {
        return set_clipboard_text_windows(text);
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = text;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard not supported on this platform",
        ))
    }
}

#[cfg(target_os = "linux")]
fn set_clipboard_text_linux(text: &str) -> io::Result<()> {
    if try_pipe_to("wl-copy", &[], text).is_ok() {
        return Ok(());
    }

    if try_pipe_to("xclip", &["-selection", "clipboard"], text).is_ok() {
        return Ok(());
    }

    if try_pipe_to("xsel", &["--clipboard", "--input"], text).is_ok() {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no clipboard tool found (tried wl-copy, xclip, xsel)",
    ))
}

#[cfg(target_os = "macos")]
fn set_clipboard_text_macos(text: &str) -> io::Result<()> {
    try_pipe_to("pbcopy", &[], text)
}

#[cfg(target_os = "windows")]
fn set_clipboard_text_windows(text: &str) -> io::Result<()> {
    // `clip.exe` reads from stdin.
    try_pipe_to("cmd", &["/C", "clip"], text)
}

fn try_pipe_to(program: &str, args: &[&str], text: &str) -> io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "missing stdin"))?;
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{program} exited with status {status}"),
        ))
    }
}

