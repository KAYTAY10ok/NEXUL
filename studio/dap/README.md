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
let launch = dap.begin_launch(serde_json::json!({
    "program": "target/debug/app.exe", "cwd": ".", "args": [], "stopOnEntry": false
}))?;
// Receive events until `initialized`, then set breakpoints.
while dap.recv_event_timeout(Duration::from_secs(10))?.event != "initialized" {}
dap.set_breakpoints(Path::new("src/main.nx"), &[Breakpoint::new(12).condition("n == 3")])?;
dap.configuration_done()?;
assert!(launch.wait(Duration::from_secs(15))?.success);
let _event = dap.recv_event_timeout(Duration::from_secs(1))?;
dap.shutdown(Duration::from_secs(2))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`DapClient` exposes `initialize`, `begin_launch`, `begin_attach`, `set_breakpoints` (including
conditions, hit conditions and logpoints), `configuration_done`, `threads`, `stack_trace`,
`scopes`, `variables`, `continue_execution`, `next`, `step_in`, `step_out`, `pause`,
`evaluate`, `disconnect`, plus event receive methods. The legacy blocking `launch` and
`attach` methods are deprecated because they can wait for a response that an adapter
defers until after configuration. Use `begin_launch` and `begin_attach` for new code.
Adapter stderr is surfaced as an
`adapterOutput` event. The protocol reader caps frames at 32 MiB and reports malformed
headers and JSON as protocol errors.

For session startup use `begin_launch` or `begin_attach`: send the request, wait for
the `initialized` event, configure breakpoints, send `configuration_done`, then poll
with `PendingRequest::try_wait()` or wait from a worker thread with
`PendingRequest::wait(timeout)`. LLDB can defer the launch response until configuration
is complete, as allowed by [DAP launch sequencing](https://microsoft.github.io/debug-adapter-protocol/overview#launch-sequencing).
Waiting for the launch response before configuring causes a timeout. The existing
blocking `launch`/`attach` methods remain available for adapters that respond immediately.

`request_async` generalizes this pattern to any request. Its handle owns the response
subscription independently of the client borrow; `wait(timeout)` starts its timeout
when called. `try_wait()` returns `None` while pending, returns a response once, then
returns `None` on later polls; a polled response remains available to `wait(timeout)`.
Dropping the handle or timing out removes the local subscription (it does not cancel the
operation in the adapter). Late replies cannot be mistaken for a later request. Adapter
exit disconnects pending requests and causes new requests to fail immediately with
`AdapterExited`. Keep blocking waits away from the GPUI thread and render
`stopped`/`output`/`exited`/`terminated` events there.

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
explicit skip reason. If `NEXUL_NXC` or `NEXUL_LLVM` is set but the corresponding tool
cannot be found, the test fails instead of silently skipping. The fake-adapter integration
test exercises framing, lifecycle, breakpoints, events, stack, scopes, variables, stepping and shutdown without requiring an
external debugger.

Run tests with `cargo test --manifest-path studio/dap/Cargo.toml`.
To require the local compiler and LLVM installation explicitly on Windows:

```powershell
$env:NEXUL_NXC = "$env:LOCALAPPDATA\Programs\NEXUL\bin\nxc.exe"
$env:NEXUL_LLVM = 'C:\Program Files\LLVM'
cargo test --manifest-path studio/dap/Cargo.toml -- --nocapture
```

The real integration test verifies a resolved breakpoint at line 3, `answer = 42`,
and stepping to line 4. The fake adapter delays launch/attach until configuration,
and covers late responses, abandoned requests and adapter exit.
