use nexul_studio_templates::{all_templates, Tool, ToolchainProvider, ToolchainRegistry};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
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

fn wait_for_http(child: &mut Child, port: u16) -> bool {
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
            stream
                .write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            let _ = stream.read_to_string(&mut response);
            return response.contains("200 OK") && response.contains("Mon site NEXUL");
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

fn run_web_server(plan: &nexul_studio_templates::CommandPlan, root: &Path, port: u16) {
    let command = &plan.commands[0];
    let mut child = command
        .command()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NEXUL_SITE_PORT", port.to_string())
        .spawn()
        .unwrap();
    let served = wait_for_http(&mut child, port);
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        served,
        "web template did not serve index.html from {} using {}",
        root.display(),
        command.program.display()
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
            let run = actions
                .run
                .iter()
                .find(|plan| {
                    let program = &plan.commands[0].program;
                    (tools.executable(Tool::Node).as_ref() == Some(program))
                        || (tools.executable(Tool::Python).as_ref() == Some(program))
                })
                .expect("site should have a Node.js or Python server plan");
            run_web_server(run, &root, port);
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
