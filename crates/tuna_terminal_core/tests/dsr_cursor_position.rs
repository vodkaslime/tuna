#![cfg(unix)]

use std::time::{Duration, Instant};

use tuna_terminal_core::pty::PtyDimensions;
use tuna_terminal_core::session::TerminalSession;

#[test]
#[ignore = "requires PTY access and an interactive-ish environment"]
fn responds_to_dsr_cursor_position() {
    let mut session =
        TerminalSession::spawn_shell(PtyDimensions::new(80, 24), 1_000).expect("spawn shell");

    // Ask the running shell to execute a tiny program that:
    // 1) switches stdin to raw mode,
    // 2) emits DSR (ESC[6n),
    // 3) reads until the response terminator 'R',
    // 4) prints DSR_OK if a response was received.
    //
    // Many terminal libraries (like crossterm) rely on this roundtrip for `cursor::position()`.
    let script = r#"python3 - <<'PY'
import sys, termios, tty
fd = sys.stdin.fileno()
old = termios.tcgetattr(fd)
tty.setraw(fd)
try:
    sys.stdout.write("\x1b[6n")
    sys.stdout.flush()
    resp = ""
    while not resp.endswith("R") and len(resp) < 64:
        resp += sys.stdin.read(1)
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, old)

if resp.startswith("\x1b[") and resp.endswith("R"):
    print("DSR_OK")
else:
    print("DSR_BAD", repr(resp))
PY
"#;

    session.write_bytes(script.as_bytes()).expect("write script");

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        session.poll_pty();

        let snap = session.snapshot();
        let mut text = String::with_capacity((snap.cols as usize + 1) * snap.rows as usize);
        for row in 0..snap.rows {
            for col in 0..snap.cols {
                text.push(snap.cell(col, row).ch);
            }
            text.push('\n');
        }

        if text.contains("DSR_OK") {
            return;
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    panic!("did not observe DSR_OK in terminal output within timeout");
}
