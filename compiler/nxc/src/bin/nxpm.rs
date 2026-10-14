//! `nxpm`: the NEXUL package manager (spec §11).

use nxc_driver::pkg::{self, Manifest, Source};
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const USAGE: &str = "\
nxpm — the NEXUL package manager, version 0.2

usage:
    nxpm new <name>                     create a project in a new folder <name>
    nxpm init [name]                    create nexul.toml in the current folder
    nxpm add <name> --path <folder>     add a dependency on a local package
    nxpm add <name> --git <url> [--tag <t> | --rev <commit> | --branch <b>]
                                        add a dependency on a Git repository
    nxpm remove <name>                  remove a dependency
    nxpm install                        fetch the dependencies (versions of nexul.lock)
    nxpm update [name]                  move Git dependencies to their newest commit
    nxpm verify                         check the integrity of the dependencies
    nxpm list [--json]                  show the dependencies
    nxpm build|run|test|check [options] [-- arguments]
                                        compile the project's main file with nxc

A project is described by nexul.toml; nexul.lock pins the exact commit and the SHA-256
checksum of every dependency (commit it with the project).
";

fn fail(msg: &str) -> ! {
    eprintln!("error: {}", msg);
    exit(1);
}

fn project() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    pkg::find_project(&cwd).unwrap_or_else(|| fail(&format!("no {} in this folder or its parents (create one with `nxpm init`)", pkg::MANIFEST)))
}

fn save(project: &Path, m: &Manifest) {
    if let Err(e) = std::fs::write(project.join(pkg::MANIFEST), pkg::write_manifest(m)) {
        fail(&format!("cannot write {}: {}", pkg::MANIFEST, e));
    }
}

fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !n.chars().next().unwrap().is_ascii_digit()
}

fn install(project: &Path, update: bool) -> Vec<pkg::Resolved> {
    match pkg::resolve(project, update, &mut |m| println!("  {}", m)) {
        Ok(r) => r,
        Err(e) => fail(&e),
    }
}

fn json_str(s: &str) -> String {
    nxc_driver::json_str(s)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_default();
    let rest = &args[args.len().min(1)..];
    match cmd.as_str() {
        "" | "help" | "--help" | "-h" => print!("{}", USAGE),
        "version" | "--version" => println!("nxpm {}", env!("CARGO_PKG_VERSION")),
        "new" | "init" => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let (dir, name) = if cmd == "new" {
                let name = rest.first().cloned().unwrap_or_else(|| fail("usage: nxpm new <name>"));
                (cwd.join(&name), name)
            } else {
                let name = rest.first().cloned().unwrap_or_else(|| cwd.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "projet".into()));
                (cwd, name)
            };
            let name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
            if !valid_name(&name) {
                fail("a package name is made of letters, digits and `_`, and does not start with a digit");
            }
            if let Err(e) = pkg::new_project(&dir, &name) {
                fail(&e);
            }
            println!("created project `{}` in {}", name, dir.display());
        }
        "add" => {
            let name = rest.first().cloned().unwrap_or_else(|| fail("usage: nxpm add <name> --path <folder> | --git <url>"));
            if !valid_name(&name) {
                fail(&format!("`{}` is not a valid package name", name));
            }
            let src = if let Some(p) = opt(rest, "--path") {
                Source::Path(p.replace('\\', "/"))
            } else if let Some(url) = opt(rest, "--git") {
                Source::Git { url, rev: opt(rest, "--rev"), tag: opt(rest, "--tag"), branch: opt(rest, "--branch") }
            } else {
                fail("give the source of the package: --path <folder> or --git <url>")
            };
            let project = project();
            let mut m = pkg::load_manifest(&project).unwrap_or_else(|e| fail(&e));
            m.deps.insert(name.clone(), src);
            save(&project, &m);
            let r = install(&project, false);
            println!("added `{}` ({} package{} locked)", name, r.len(), if r.len() == 1 { "" } else { "s" });
        }
        "remove" => {
            let name = rest.first().cloned().unwrap_or_else(|| fail("usage: nxpm remove <name>"));
            let project = project();
            let mut m = pkg::load_manifest(&project).unwrap_or_else(|e| fail(&e));
            if m.deps.remove(&name).is_none() {
                fail(&format!("`{}` is not a dependency of this project", name));
            }
            save(&project, &m);
            install(&project, false);
            println!("removed `{}`", name);
        }
        "install" | "update" => {
            let project = project();
            let r = install(&project, cmd == "update");
            for d in &r {
                println!("{:<16} {}", d.name, d.source);
            }
            println!("{} package{} ready", r.len(), if r.len() == 1 { "" } else { "s" });
        }
        "verify" => {
            let project = project();
            let lock = pkg::read_lock(&project);
            let r = install(&project, false);
            let mut bad = 0;
            for d in &r {
                let ok = lock.get(&d.name).map_or(true, |(_, s)| s.is_empty() || *s == d.checksum || d.source.starts_with("path+"));
                println!("{} {:<16} {}", if ok { "ok " } else { "BAD" }, d.name, d.checksum);
                if !ok {
                    bad += 1;
                }
            }
            if bad > 0 {
                exit(1);
            }
        }
        "list" => {
            let project = project();
            let m = pkg::load_manifest(&project).unwrap_or_else(|e| fail(&e));
            let lock = pkg::read_lock(&project);
            if rest.iter().any(|a| a == "--json") {
                let deps: Vec<String> = m
                    .deps
                    .iter()
                    .map(|(n, s)| {
                        let (kind, loc) = match s {
                            Source::Path(p) => ("path", p.clone()),
                            Source::Git { url, tag, rev, branch } => {
                                let at = tag.clone().or_else(|| rev.clone()).or_else(|| branch.clone()).map(|x| format!("@{}", x)).unwrap_or_default();
                                ("git", format!("{}{}", url, at))
                            }
                        };
                        let (src, sum) = lock.get(n).cloned().unwrap_or_default();
                        format!(
                            "{{\"name\":{},\"kind\":\"{}\",\"location\":{},\"locked\":{},\"checksum\":{}}}",
                            json_str(n),
                            kind,
                            json_str(&loc),
                            json_str(&src),
                            json_str(&sum)
                        )
                    })
                    .collect();
                let main = pkg::main_file(&project, &m);
                println!(
                    "{{\"project\":{},\"name\":{},\"version\":{},\"main\":{},\"dependencies\":[{}]}}",
                    json_str(&project.to_string_lossy()),
                    json_str(&m.name),
                    json_str(&m.version),
                    json_str(&main.to_string_lossy()),
                    deps.join(",")
                );
            } else {
                println!("{} {}", m.name, m.version);
                for (n, s) in &m.deps {
                    let locked = lock.get(n).map(|(src, _)| src.clone()).unwrap_or_else(|| "(not installed)".into());
                    println!("  {:<16} {:?}  {}", n, s, locked);
                }
            }
        }
        "build" | "run" | "test" | "check" => {
            let project = project();
            let m = pkg::load_manifest(&project).unwrap_or_else(|e| fail(&e));
            install(&project, false);
            let main = pkg::main_file(&project, &m);
            let nxc = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(if cfg!(windows) { "nxc.exe" } else { "nxc" }))).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("nxc"));
            let mut c = Command::new(nxc);
            c.arg(&cmd).arg(&main);
            let mut extra: Vec<String> = rest.to_vec();
            if cmd == "build" && !extra.iter().any(|a| a == "-o") {
                let target = project.join("target");
                let _ = std::fs::create_dir_all(&target);
                let exe = target.join(if cfg!(windows) { format!("{}.exe", m.name) } else { m.name.clone() });
                extra.push("-o".into());
                extra.push(exe.to_string_lossy().to_string());
            }
            c.args(&extra).current_dir(&project);
            match c.status() {
                Ok(s) => exit(s.code().unwrap_or(1)),
                Err(e) => fail(&format!("cannot run nxc: {}", e)),
            }
        }
        other => fail(&format!("unknown command `{}`\n\n{}", other, USAGE)),
    }
}
