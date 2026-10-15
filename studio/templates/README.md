# `nexul-studio-templates`

Standalone, GPUI-independent project-template catalog and instantiator.

## Templates

- NEXUL console application.
- NEXUL library module with a runnable example.
- `nxpm` application with a checked-in local package dependency.
- NEXUL `wasm32-web` browser library with the ABI v1 loader and local preview servers.
- HTML/CSS/JavaScript site with local Node.js and Python servers.
- Python, C, C++, Rust/Cargo, and C#/.NET console applications.

Each descriptor includes its files, tool requirements, build plan and run plan. C and C++
select MSVC, Clang, GCC, Clang++, or G++ from the installed tool paths supplied by Studio.
The site is available when Node.js or Python is present. Projects are created without
overwriting existing files and the writer rejects rooted or parent-traversal paths.

## SDK catalog boundary

Production code should implement `ToolchainProvider` by adapting the installed SDK
catalog from `studio/lsp`. This crate intentionally has no `PATH` scanner and does not
download anything. The `ToolchainRegistry` is a plain data object useful for bridging
that catalog into template availability and command generation.

The WebAssembly template appears only when the supplied toolchain catalog confirms that
`nxc targets` advertises `wasm32-web`, and Node.js or Python is available to serve the
project over HTTP. The generated `nexul.mjs` comes from the canonical
`lib/web/nexul.mjs` source; this crate does not maintain a second loader implementation.

For target-dependent templates, the Studio adapter should implement
`ToolchainProvider::supports_target` using the tool's target query rather than a version
number. Cache that result in the SDK catalog (or `ToolchainRegistry::set_target_support`)
so refreshing template availability does not spawn `nxc` repeatedly. The default
implementation returns unknown, so older adapters do not advertise the browser template
prematurely.

```rust,no_run
use nexul_studio_templates::{find_template, Tool, ToolchainRegistry};
use std::path::PathBuf;

let mut sdk = ToolchainRegistry::default(); // populate from studio/lsp
sdk.insert(Tool::Nxc, PathBuf::from("/opt/nexul/bin/nxc"));
let template = find_template("nexul-console").unwrap();
let destination = PathBuf::from("hello-nexul");
template.create(&destination)?;
let actions = template.actions(&sdk, &destination)?;
let build = actions.build.expect("build command");
for command in build.commands { command.run()?; }
# Ok::<(), Box<dyn std::error::Error>>(())
```

Run unit and toolchain-conditional functional tests with `cargo test --manifest-path studio/templates/Cargo.toml`.
