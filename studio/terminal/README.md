# `nexul-studio-terminal`

Standalone PTY and screen-model crate for NEXUL Studio. It has no GPUI dependency.
`portable-pty` provides the platform PTY (ConPTY on Windows and native PTYs on Unix),
while `vte` parses VT/ANSI output into a cell grid.

## API

```rust,no_run
use nexul_studio_terminal::{ShellSpec, Terminal, TerminalEvent};
use std::{path::Path, time::Duration};

let shell = ShellSpec::default_shell()?;
let mut terminal = Terminal::spawn(shell, Path::new("."), 100, 30)?;
terminal.write_text("help\r\n")?;
let snapshot = terminal.snapshot();
for line in snapshot.lines() {
    // Render each cell's text, colors and bold/italic/underline style in the UI.
}
if let Ok(TerminalEvent::Output(_)) = terminal.recv_event_timeout(Duration::from_millis(50)) {
    // Refresh the view.
}
terminal.resize(120, 36)?;
terminal.stop()?;
# Ok::<(), std::io::Error>(())
```

`ShellSpec::powershell`, `cmd`, `git_bash`, and `wsl` create the Windows shell choices;
`default_shell` uses PowerShell on Windows and `$SHELL` (falling back to `sh`) on Unix.
`TerminalManager` can own several independently running PTYs. UI code owns the
selection coordinates and calls `set_selection`; `selected_text` returns its text.

## Behavior and limits

- Input is raw terminal input; callers translate keyboard and paste events into the
  appropriate bytes for their chosen shell.
- The screen model supports cursor movement, scrolling, line/display erase, insertion and
  deletion, SGR basic/bright/indexed/RGB colors, bold, italic, underline, OSC titles,
  Unicode combining marks and double-width characters, and a bounded scrollback buffer.
- The model intentionally does not implement a complete terminal UI (mouse reporting,
  alternate screen buffer, hyperlinks, image protocols, or terminal-specific extensions).
- Terminal creation fails with a clear I/O error when the selected shell cannot be found
  or the PTY cannot be opened.

Run the crate tests with `cargo test --manifest-path studio/terminal/Cargo.toml`.
