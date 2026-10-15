use nexul_studio_templates::{all_templates, find_template, Tool, ToolchainProvider, ToolchainRegistry};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct EnvironmentTools(ToolchainRegistry);
impl ToolchainProvider for EnvironmentTools {
    fn executable(&self, tool: Tool) -> Option<PathBuf> {
        self.0.executable(tool)
    }
}

fn find_on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for name in names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
            #[cfg(windows)]
            for extension in [".exe", ".cmd", ".bat"] {
                let candidate = directory.join(format!("{name}{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn sdk_catalog() -> EnvironmentTools {
    // Test harness discovery only; production callers should pass studio/lsp's catalog.
    let mut tools = ToolchainRegistry::default();
    let candidates: &[(Tool, &[&str])] = &[
        (Tool::Nxc, &["nxc.exe", "nxc"]),
        (Tool::Nxpm, &["nxpm.exe", "nxpm"]),
        (Tool::Python, &["python.exe", "python", "python3"]),
        (Tool::Node, &["node.exe", "node"]),
        (Tool::Gcc, &["gcc.exe", "gcc"]),
        (Tool::Clang, &["clang.exe", "clang"]),
        (Tool::Cl, &["cl.exe", "cl"]),
        (Tool::Gxx, &["g++.exe", "g++"]),
        (Tool::Clangxx, &["clang++.exe", "clang++"]),
        (Tool::Cargo, &["cargo.exe", "cargo"]),
        (Tool::Rustc, &["rustc.exe", "rustc"]),
        (Tool::Dotnet, &["dotnet.exe", "dotnet"]),
    ];
    for (tool, names) in candidates {
        if let Some(path) = find_on_path(names) {
            tools.insert(*tool, path);
        }
    }
    if let Some(nxc) = tools.get(Tool::Nxc).map(Path::to_path_buf) {
        if let Ok(output) = Command::new(nxc).arg("targets").output() {
            if output.status.success() {
                let targets = String::from_utf8_lossy(&output.stdout);
                let supports_web = targets
                    .lines()
                    .any(|line| line.split_whitespace().next() == Some("wasm32-web"));
                tools.set_target_support(Tool::Nxc, "wasm32-web", supports_web);
            }
        }
    }
    EnvironmentTools(tools)
}

fn run_checked(plan: &nexul_studio_templates::CommandPlan) {
    for command in &plan.commands {
        let output = command
            .command()
            .output()
            .unwrap_or_else(|error| panic!("could not run {}: {error}", command.program.display()));
        assert!(
            output.status.success(),
            "{} failed: {}\n{}",
            command.program.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn wait_for_http(
    child: &mut Child,
    port: u16,
    path: &str,
    expected_content_type: &str,
    expected_body: &[u8],
) -> bool {
    let address = ("127.0.0.1", port)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        if child.try_wait().ok().flatten().is_some() {
            return false;
        }
        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
            write!(stream, "GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response);
            let Some(separator) = response.windows(4).position(|bytes| bytes == b"\r\n\r\n")
            else {
                return false;
            };
            let headers = String::from_utf8_lossy(&response[..separator]);
            let mime_matches = headers.lines().any(|line| {
                line.split_once(':').is_some_and(|(name, value)| {
                    name.eq_ignore_ascii_case("content-type")
                        && value.trim().starts_with(expected_content_type)
                })
            });
            let body = &response[separator + 4..];
            return headers.starts_with("HTTP/1.")
                && headers.contains(" 200 ")
                && mime_matches
                && body
                    .windows(expected_body.len())
                    .any(|bytes| bytes == expected_body);
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

fn run_web_server_asset(
    plan: &nexul_studio_templates::CommandPlan,
    root: &Path,
    port: u16,
    path: &str,
    expected_content_type: &str,
    expected_body: &[u8],
) {
    let command = &plan.commands[0];
    let mut child = command
        .command()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NEXUL_SITE_PORT", port.to_string())
        .spawn()
        .unwrap();
    let served = wait_for_http(
        &mut child,
        port,
        path,
        expected_content_type,
        expected_body,
    );
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        served,
        "web template did not serve {path} from {} using {}",
        root.display(),
        command.program.display()
    );
}

fn run_nexul_web_module_with_node(node: &Path, root: &Path) {
    let verification = r#"import { readFile } from 'node:fs/promises';
import { loadNexul } from './nexul.mjs';

let nx;
let greeting;
nx = await loadNexul(await readFile('./build/calcul.wasm'), { imports: { app: {
  host_double: value => value * 2,
  host_text: (pointer, length) => { greeting = nx.readText(pointer, length); },
} } });
const value = nx.call('calculate', 20.5);
nx.withText('NEXUL 🌍 — été', (pointer, length) => nx.call('greet', pointer, length));
if (value !== 42 || greeting !== 'Bonjour, NEXUL 🌍 — été !') {
  throw new Error(`unexpected result: ${value}; ${greeting}`);
}
"#;
    fs::write(root.join("verify-web.mjs"), verification).unwrap();
    let output = Command::new(node)
        .args(["verify-web.mjs"])
        .current_dir(root)
        .output()
        .unwrap_or_else(|error| panic!("could not launch Node.js: {error}"));
    assert!(
        output.status.success(),
        "NEXUL WebAssembly ABI check failed\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn every_template_materializes_all_declared_files() {
    for template in all_templates() {
        let root = nexul_studio_templates::unique_temp_dir(template.id.as_str()).unwrap();
        template
            .create(&root)
            .unwrap_or_else(|error| panic!("{}: {error}", template.id.as_str()));
        for file in template.files() {
            assert!(
                root.join(&file.path).is_file(),
                "missing {}",
                file.path.display()
            );
        }
        if template.id.as_str() == "nexul-webassembly" {
            assert_eq!(
                fs::read_to_string(root.join("nexul.mjs")).unwrap(),
                include_str!("../../../lib/web/nexul.mjs")
            );
            assert!(fs::read_to_string(root.join("main.mjs"))
                .unwrap()
                .contains("loadNexul('./build/calcul.wasm'"));
        }
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn installed_template_toolchains_build_and_run_their_projects() {
    let tools = sdk_catalog();
    for template in all_templates()
        .into_iter()
        .filter(|template| template.is_available(&tools))
    {
        let root = nexul_studio_templates::unique_temp_dir(template.id.as_str()).unwrap();
        template
            .create(&root)
            .unwrap_or_else(|error| panic!("{} create: {error}", template.id.as_str()));
        let actions = template
            .actions(&tools, &root)
            .unwrap_or_else(|error| panic!("{} actions: {error}", template.id.as_str()));
        if let Some(build) = &actions.build {
            run_checked(build);
        }
        if template.id.as_str() == "web-site" {
            let port = 39_000 + (std::process::id() % 20_000) as u16;
            fs::write(
                root.join("main.mjs"),
                "import { loadNexul } from './nexul.mjs';\n",
            )
            .unwrap();
            fs::create_dir_all(root.join("build")).unwrap();
            fs::write(root.join("build/test.wasm"), b"\0asm\x01\0\0\0").unwrap();
            for (index, server) in actions.run.iter().enumerate() {
                let server_port = port + (index as u16 * 3);
                run_web_server_asset(
                    server,
                    &root,
                    server_port,
                    "/",
                    "text/html",
                    b"Mon site NEXUL",
                );
                run_web_server_asset(
                    server,
                    &root,
                    server_port + 1,
                    "/main.mjs",
                    "text/javascript",
                    b"loadNexul",
                );
                run_web_server_asset(
                    server,
                    &root,
                    server_port + 2,
                    "/build/test.wasm",
                    "application/wasm",
                    b"\0asm",
                );
            }
        } else if template.id.as_str() == "nexul-webassembly" {
            if let Some(node) = tools.executable(Tool::Node) {
                let syntax = Command::new(&node)
                    .args(["--check", "main.mjs"])
                    .current_dir(&root)
                    .output()
                    .unwrap();
                assert!(
                    syntax.status.success(),
                    "main.mjs syntax check failed: {}",
                    String::from_utf8_lossy(&syntax.stderr)
                );
                run_nexul_web_module_with_node(&node, &root);
            } else {
                eprintln!("skip Node.js ABI validation for nexul-webassembly: Node.js not found");
            }

            let port = 39_000 + (std::process::id() % 20_000) as u16;
            let server = actions
                .run
                .iter()
                .find(|plan| {
                    let program = &plan.commands[0].program;
                    (tools.executable(Tool::Node).as_ref() == Some(program))
                        || (tools.executable(Tool::Python).as_ref() == Some(program))
                })
                .expect("WebAssembly template should have a Node.js or Python server plan");
            run_web_server_asset(
                server,
                &root,
                port,
                "/main.mjs",
                "text/javascript",
                b"loadNexul",
            );
            let wasm = fs::read(root.join("build/calcul.wasm")).unwrap();
            assert!(wasm.starts_with(b"\0asm"), "compiler did not emit a WebAssembly module");
            run_web_server_asset(
                server,
                &root,
                port,
                "/build/calcul.wasm",
                "application/wasm",
                b"\0asm",
            );
        } else {
            let run = actions
                .run
                .first()
                .unwrap_or_else(|| panic!("{} has no run command", template.id.as_str()));
            run_checked(run);
        }
        let _ = fs::remove_dir_all(root);
    }
}

#[allow(dead_code)]
fn _os_string_anchor(_: OsString) {}
