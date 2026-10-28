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

Read `TerminalEvent`s until the queue is empty on each UI refresh; startup output and
title changes can precede `Exited`. Output is parsed before its event is sent, and
`Exited` is sent once, after the child has terminated and its final output has been
drained. The screen snapshot remains available after exit. `stop()` kills the shell
and joins the process watcher and reader; repeated calls (including `Drop`) are safe.
Run these blocking lifecycle calls away from the UI thread. Writes and resizes after
shutdown return `BrokenPipe`. Removing a terminal through `TerminalManager::close`
performs the same cleanup.

The reader queues cursor-position/status responses to a dedicated writer thread,
including ConPTY's initial cursor query. It never waits on the input lock while it
drains output. A separate process watcher waits for the child without holding a lock
needed by `stop()`, then closes the input and PTY while the reader drains output. This
also handles a shell exiting normally without an explicit `stop()` call.

On Unix, `stop()` terminates the configured shell process. A background descendant
that inherited the PTY can keep its output open, so shutdown may wait until that
descendant exits or closes the PTY. Call `stop()` away from the UI thread; applications
that launch long-lived background jobs should manage those jobs before stopping the
terminal.

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

On Windows, use the installed **MSVC** toolchain and Visual Studio C++ tools:

```powershell
cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/terminal/Cargo.toml
```

The default Windows GNU toolchain additionally needs MinGW's `dlltool` for
`windows-sys`; MSVC avoids that prerequisite. On Unix, use
`cargo test --manifest-path studio/terminal/Cargo.toml`. The tests exercise a real
interactive shell, resize/stop, normal/nonzero exit and output draining, as well as
the screen model. Windows tests require PowerShell and `cmd.exe`; Unix tests use
`$SHELL` and `/bin/sh`.
