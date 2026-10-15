# `nexul-studio-dap`

Standalone Debug Adapter Protocol client for NEXUL Studio. The crate has no GPUI
dependency and uses a reader thread, a synchronized request writer, and an event channel.
The transport follows DAP's `Content-Length` framing on adapter stdin/stdout.

## Starting an adapter

```rust,no_run
use nexul_studio_dap::{AdapterKind, AdapterRegistry, Breakpoint, DapClient};
use std::{path::Path, time::Duration};

let adapters = AdapterRegistry::discover(None); // detection only; never downloads
let command = adapters.get(AdapterKind::LldbDap).expect("install LLVM / NEXUL");
let mut dap = DapClient::spawn(command)?;
dap.initialize("nexul-studio", "NEXUL Studio")?;
dap.launch(serde_json::json!({
    "program": "target/debug/app.exe", "cwd": ".", "args": [], "stopOnEntry": false
}))?;
// Wait for `initialized`, set breakpoints, then send configuration_done().
dap.set_breakpoints(Path::new("src/main.nx"), &[Breakpoint::new(12).condition("n == 3")])?;
dap.configuration_done()?;
let _event = dap.recv_event_timeout(Duration::from_secs(1))?;
dap.shutdown(Duration::from_secs(2))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`DapClient` exposes `initialize`, `launch`, `attach`, `set_breakpoints` (including
conditions, hit conditions and logpoints), `configuration_done`, `threads`, `stack_trace`,
`scopes`, `variables`, `continue_execution`, `next`, `step_in`, `step_out`, `pause`,
`evaluate`, `disconnect`, plus event receive methods. Adapter stderr is surfaced as an
`adapterOutput` event. The protocol reader caps frames at 32 MiB and reports malformed
headers and JSON as protocol errors.

## Adapter registry

`AdapterRegistry::discover` locates `lldb-dap` (including `NEXUL_LLVM`, LLVM's standard
Windows path and the NEXUL installation layout), `debugpy` in an already-installed Python,
`netcoredbg`, and Microsoft's installed VS Code JavaScript debug adapter. Detection does
not install or download tools. The JavaScript adapter requires Node.js and an installed
`ms-vscode.js-debug` adapter script or a `js-debug-adapter` executable.

For NEXUL, `build_nexul_program` invokes `nxc build` without `--release`, which uses the
compiler's default debug information, and verifies the executable and Windows PDB before
returning. Use `lldb_launch_arguments` to build the LLDB launch request. The NEXUL + LLDB
integration test runs when both `nxc` and `lldb-dap` are installed; otherwise it prints an
explicit skip reason. The fake-adapter integration test exercises framing, lifecycle,
breakpoints, events, stack, scopes, variables, stepping and shutdown without requiring an
external debugger.

Run tests with `cargo test --manifest-path studio/dap/Cargo.toml`.
