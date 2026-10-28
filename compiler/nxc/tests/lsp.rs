use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);
static SERIAL: Mutex<()> = Mutex::new(());

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nxc-lsp-{}-{}", std::process::id(), id));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Client {
    child: Child,
    input: Option<ChildStdin>,
    messages: Receiver<String>,
    seen: Vec<String>,
}

impl Client {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nxc"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, messages) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut length = None;
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => return,
                        Ok(_) => {}
                        Err(_) => return,
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            length = value.trim().parse::<usize>().ok();
                        }
                    }
                }
                let Some(length) = length else { return };
                let mut body = vec![0; length];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                if send
                    .send(String::from_utf8_lossy(&body).into_owned())
                    .is_err()
                {
                    return;
                }
            }
        });
        Self {
            input: child.stdin.take(),
            child,
            messages,
            seen: Vec::new(),
        }
    }

    fn send(&mut self, json: &str) {
        let input = self.input.as_mut().expect("client stdin is open");
        write!(input, "Content-Length: {}\r\n\r\n{}", json.len(), json).unwrap();
        input.flush().unwrap();
    }

    fn request(&mut self, id: usize, method: &str, params: &str) {
        self.send(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":{},\"params\":{}}}",
            id,
            quote(method),
            params
        ));
    }

    fn notification(&mut self, method: &str, params: &str) {
        self.send(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":{},\"params\":{}}}",
            quote(method),
            params
        ));
    }

    fn matching(&mut self, predicate: impl Fn(&str) -> bool, timeout: Duration) -> String {
        if let Some(index) = self.seen.iter().position(|message| predicate(message)) {
            return self.seen.remove(index);
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let message = self
                .messages
                .recv_timeout(remaining)
                .unwrap_or_else(|error| panic!("timed out waiting for LSP message: {error}"));
            if predicate(&message) {
                return message;
            }
            self.seen.push(message);
        }
    }

    fn response(&mut self, id: usize) -> String {
        self.matching(
            |message| message.contains(&format!("\"id\":{}", id)),
            Duration::from_secs(20),
        )
    }

    fn timed_request(&mut self, id: usize, method: &str, params: &str) -> (String, Duration) {
        let started = Instant::now();
        self.request(id, method, params);
        let response = self.response(id);
        (response, started.elapsed())
    }

    fn diagnostics(&mut self, uri: &str, version: usize) -> String {
        self.matching(
            |message| {
                message.contains("textDocument/publishDiagnostics")
                    && message.contains(uri)
                    && message.contains(&format!("\"version\":{}", version))
            },
            Duration::from_secs(20),
        )
    }

    fn diagnostics_without_version(&mut self, uri: &str) -> String {
        self.matching(
            |message| {
                message.contains("textDocument/publishDiagnostics")
                    && message.contains(uri)
                    && message.contains("nxpm install")
            },
            Duration::from_secs(20),
        )
    }

    fn exit(mut self, shutdown: bool) -> i32 {
        if shutdown {
            self.request(90, "shutdown", "{}");
            let response = self.response(90);
            assert!(response.contains("\"result\":null"), "{response}");
        }
        self.notification("exit", "{}");
        self.input.take();
        self.child.wait().unwrap().code().unwrap()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        format!("file:///{}", path.trim_start_matches('/'))
    } else {
        format!("file://{}", path)
    }
}

#[cfg(windows)]
fn process_working_set(pid: u32) -> Option<u64> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-Command",
            &format!("(Get-Process -Id {}).WorkingSet64", pid),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

fn initialize(client: &mut Client, id: usize, encodings: &str) -> String {
    client.request(
        id,
        "initialize",
        &format!(
            "{{\"capabilities\":{{\"general\":{{\"positionEncodings\":{}}}}}}}",
            encodings
        ),
    );
    client.response(id)
}

#[test]
fn lsp_lifecycle_and_position_encoding_negotiation() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let mut client = Client::start();
    let initialized = initialize(&mut client, 1, "[\"utf-16\",\"utf-8\"]");
    assert!(
        initialized.contains("\"positionEncoding\":\"utf-8\""),
        "{initialized}"
    );
    assert!(initialized.contains("\"change\":2"), "{initialized}");
    assert!(
        initialized.contains("\"prepareProvider\":true"),
        "{initialized}"
    );
    assert_eq!(client.exit(true), 0);

    let mut client = Client::start();
    initialize(&mut client, 1, "[]");
    assert_eq!(client.exit(false), 1);

    let mut child = Command::new(env!("CARGO_BIN_EXE_nxc"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    assert_eq!(child.wait().unwrap().code(), Some(0));
}

#[test]
fn lsp_requests_cover_open_project_documents_and_standard_library() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.path().join("src/tools")).unwrap();
    std::fs::write(
        scratch.path().join("nexul.toml"),
        "[package]\nname = \"lsp-case\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let main_path = scratch.path().join("src/main.nx");
    let calc_path = scratch.path().join("src/tools/calc.nx");
    let main = "import tools.calc\nimport std.collections.{Map}\nstruct LocalRecord { field: i32 }\nfn main() {\n    let _map: Map[String, i32] = Map.new()\n    let record = LocalRecord(field: 1)\n    let _field = record.field\n    let _answer: i32 = calc.value()\n}\n@test\nfn sample() {}\n";
    let calc = "pub fn value() -> i32 { return 1 }\n";
    std::fs::write(&main_path, main).unwrap();
    std::fs::write(&calc_path, calc).unwrap();
    let main_uri = uri(&main_path);
    let calc_uri = uri(&calc_path);

    let mut client = Client::start();
    let initialized = initialize(&mut client, 1, "[\"utf-16\"]");
    assert!(initialized.contains("\"positionEncoding\":\"utf-16\""));
    client.notification("initialized", "{}");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"nexul\",\"version\":1,\"text\":{}}}}}",
            quote(&main_uri),
            quote(main)
        ),
    );
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"nexul\",\"version\":1,\"text\":{}}}}}",
            quote(&calc_uri),
            quote(calc)
        ),
    );

    let call_line_index = main
        .lines()
        .position(|line| line.contains("calc.value()"))
        .unwrap();
    let call_line = main.lines().nth(call_line_index).unwrap();
    let call_character = call_line.find("value").unwrap();
    let call_params = format!(
        "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}}}}",
        quote(&main_uri),
        call_line_index,
        call_character
    );
    let completion_line = main
        .lines()
        .position(|line| line.contains("record.field"))
        .unwrap();
    let completion_character = main
        .lines()
        .find(|line| line.contains("record.field"))
        .unwrap()
        .find("record.")
        .unwrap()
        + "record.".len();
    let direct_analysis = nxc_driver::ide::analyze_document_with_cfg(
        &main_path,
        Some(main.to_string()),
        vec![(calc_path.clone(), calc.to_string())],
        nxc_sema::target::Cfg::default(),
    )
    .unwrap();
    let direct_completion = nxc_driver::ide::completion_with_analysis(
        &direct_analysis,
        main,
        completion_line + 1,
        completion_character + 1,
    );
    assert!(
        direct_completion.contains("\"label\":\"field\""),
        "{direct_completion}"
    );
    client.request(2, "textDocument/hover", &call_params);
    assert!(client.response(2).contains("```nexul"));
    client.request(3, "textDocument/definition", &call_params);
    let definition = client.response(3);
    assert!(definition.contains(&calc_uri), "{definition}");
    client.request(
        4,
        "textDocument/references",
        &format!(
            "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}},\"context\":{{\"includeDeclaration\":true}}}}",
            quote(&main_uri),
            call_line_index,
            call_character
        ),
    );
    let references = client.response(4);
    assert!(
        references.contains(&main_uri) && references.contains(&calc_uri),
        "{references}"
    );
    client.request(5, "textDocument/prepareRename", &call_params);
    let prepare = client.response(5);
    assert!(prepare.contains("\"placeholder\":\"value\""), "{prepare}");
    client.request(
        6,
        "textDocument/rename",
        &format!(
            "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}},\"newName\":\"renamed_value\"}}",
            quote(&main_uri),
            call_line_index,
            call_character
        ),
    );
    let rename = client.response(6);
    assert!(
        rename.contains("renamed_value") && rename.contains(&calc_uri),
        "{rename}"
    );
    client.request(
        7,
        "textDocument/rename",
        &format!(
            "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}},\"newName\":\"let\"}}",
            quote(&main_uri),
            call_line_index,
            call_character
        ),
    );
    assert!(client.response(7).contains("\"code\":-32602"));
    client.request(
        8,
        "textDocument/completion",
        &format!(
            "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}}}}",
            quote(&main_uri),
            completion_line,
            completion_character
        ),
    );
    let completion = client.response(8);
    assert!(completion.contains("\"label\":\"field\""), "{completion}");
    client.request(
        9,
        "textDocument/documentSymbol",
        &format!("{{\"textDocument\":{{\"uri\":{}}}}}", quote(&main_uri)),
    );
    let symbols = client.response(9);
    assert!(
        symbols.contains("\"name\":\"main\"") && symbols.contains("\"name\":\"sample\""),
        "{symbols}"
    );
    client.request(
        10,
        "nexul/tests",
        &format!("{{\"textDocument\":{{\"uri\":{}}}}}", quote(&main_uri)),
    );
    assert!(client.response(10).contains("sample"));
    client.request(
        11,
        "textDocument/formatting",
        &format!("{{\"textDocument\":{{\"uri\":{}}}}}", quote(&main_uri)),
    );
    assert!(client.response(11).contains("\"result\":"));

    let map_line_index = main
        .lines()
        .position(|line| line.contains("Map[String"))
        .unwrap();
    let map_line = main.lines().nth(map_line_index).unwrap();
    client.request(
        12,
        "textDocument/definition",
        &format!(
            "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}}}}",
            quote(&main_uri),
            map_line_index,
            map_line.find("Map").unwrap()
        ),
    );
    let std_definition = client.response(12);
    assert!(
        std_definition.contains("nexul-std:///collections.nx"),
        "{std_definition}"
    );
    client.request(13, "nexul/stdSource", "{\"uri\":\"nexul-std:///vec.nx\"}");
    let std_source = client.response(13);
    assert!(
        std_source.contains("\"text\"") && std_source.contains("struct Vec"),
        "{std_source}"
    );

    let initial_main_diagnostics = client.diagnostics(&main_uri, 1);
    let initial_calc_diagnostics = client.diagnostics(&calc_uri, 1);
    assert!(
        initial_main_diagnostics.contains("\"diagnostics\":[]"),
        "{initial_main_diagnostics}"
    );
    assert!(
        initial_calc_diagnostics.contains("\"diagnostics\":[]"),
        "{initial_calc_diagnostics}"
    );

    let incompatible = "pub fn value() -> String { return \"changed\" }\n";
    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":2}},\"contentChanges\":[{{\"text\":{}}}]}}",
            quote(&calc_uri),
            quote(incompatible)
        ),
    );
    let bad_diagnostics = client.diagnostics(&main_uri, 1);
    assert!(
        bad_diagnostics.contains("\"diagnostics\":[{"),
        "{bad_diagnostics}"
    );
    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":3}},\"contentChanges\":[{{\"text\":{}}}]}}",
            quote(&calc_uri),
            quote(calc)
        ),
    );
    let fixed_diagnostics = client.diagnostics(&main_uri, 1);
    assert!(
        fixed_diagnostics.contains("\"diagnostics\":[]"),
        "{fixed_diagnostics}"
    );

    client.request(14, "workspace/unknownMethod", "{}");
    assert!(client.response(14).contains("\"code\":-32601"));
    client.send("{");
    assert!(client
        .matching(
            |message| message.contains("\"code\":-32700"),
            Duration::from_secs(5)
        )
        .contains("Parse error"));
    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":4}},\"contentChanges\":[{{\"range\":{{\"start\":{{\"line\":999,\"character\":0}},\"end\":{{\"line\":999,\"character\":1}}}},\"text\":\"x\"}}]}}",
            quote(&calc_uri)
        ),
    );
    client.request(
        15,
        "textDocument/documentSymbol",
        &format!("{{\"textDocument\":{{\"uri\":{}}}}}", quote(&calc_uri)),
    );
    assert!(client.response(15).contains("\"name\":\"value\""));
    assert_eq!(client.exit(true), 0);
}

#[test]
fn lsp_uses_each_open_project_module_edition_for_diagnostics() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.path().join("src/tools")).unwrap();
    std::fs::write(
        scratch.path().join("nexul.toml"),
        "[package]\nname = \"lsp-mixed-module-editions\"\nedition = \"1\"\n",
    )
    .unwrap();

    let main_path = scratch.path().join("src/main.nx");
    let module_path = scratch.path().join("src/tools/calc.nx");
    let main = "// edition: 1\nimport tools.calc\nfn main() {\n    let i32 answer = calc.carre(4)\n    println(\"{answer}\")\n}\n";
    let module = "// edition: 0.2\npub fn carre(x: i32) -> i32 { return x * x }\n";
    std::fs::write(&main_path, main).unwrap();
    std::fs::write(&module_path, module).unwrap();
    let main_uri = uri(&main_path);
    let module_uri = uri(&module_path);

    let mut client = Client::start();
    initialize(&mut client, 1, "[]");
    client.notification("initialized", "{}");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"nexul\",\"version\":1,\"text\":{}}}}}",
            quote(&main_uri),
            quote(main)
        ),
    );
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"nexul\",\"version\":1,\"text\":{}}}}}",
            quote(&module_uri),
            quote(module)
        ),
    );

    let main_diagnostics = client.diagnostics(&main_uri, 1);
    let module_diagnostics = client.diagnostics(&module_uri, 1);
    assert!(
        main_diagnostics.contains("\"diagnostics\":[]"),
        "edition-1 project diagnostics: {main_diagnostics}"
    );
    assert!(
        module_diagnostics.contains("\"diagnostics\":[]"),
        "edition-0.2 module diagnostics: {module_diagnostics}"
    );
    assert_eq!(client.exit(true), 0);
}

#[test]
fn lsp_uses_cached_dependency_package_edition_in_edition_one_project() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.path().join("legacy/src")).unwrap();
    std::fs::create_dir_all(scratch.path().join("app/src")).unwrap();
    std::fs::write(
        scratch.path().join("legacy/nexul.toml"),
        "[package]\nname = \"legacy\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.path().join("legacy/src/lib.nx"),
        "pub fn carre(x: i32) -> i32 { return x * x }\n",
    )
    .unwrap();
    std::fs::write(
        scratch.path().join("app/nexul.toml"),
        "[package]\nname = \"lsp-edition-one-app\"\nedition = \"1\"\n\n[dependencies]\nlegacy = { path = \"../legacy\" }\n",
    )
    .unwrap();

    let main_path = scratch.path().join("app/src/main.nx");
    let dependency_path = scratch.path().join("legacy/src/lib.nx");
    let main = "// edition: 1\nimport legacy\nfn main() {\n    let i32 answer = legacy.carre(5)\n    println(\"{answer}\")\n}\n";
    let dependency = std::fs::read_to_string(&dependency_path).unwrap();
    std::fs::write(&main_path, main).unwrap();
    let main_uri = uri(&main_path);
    let dependency_uri = uri(&dependency_path);

    let mut client = Client::start();
    initialize(&mut client, 1, "[]");
    client.notification("initialized", "{}");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"nexul\",\"version\":1,\"text\":{}}}}}",
            quote(&main_uri),
            quote(main)
        ),
    );
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"nexul\",\"version\":1,\"text\":{}}}}}",
            quote(&dependency_uri),
            quote(&dependency)
        ),
    );

    let main_diagnostics = client.diagnostics(&main_uri, 1);
    let dependency_diagnostics = client.diagnostics(&dependency_uri, 1);
    assert!(
        main_diagnostics.contains("\"diagnostics\":[]"),
        "edition-1 app diagnostics with edition-0.2 dependency: {main_diagnostics}"
    );
    assert!(
        dependency_diagnostics.contains("\"diagnostics\":[]"),
        "edition-0.2 dependency diagnostics: {dependency_diagnostics}"
    );
    assert_eq!(client.exit(true), 0);
}

#[test]
fn lsp_header_edition_changes_recompute_diagnostics() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let scratch = Scratch::new();
    let path = scratch.path().join("changing-edition.nx");
    let document_uri = uri(&path);
    let edition_02 = "fn answer() -> i32 { return 42 }\n";
    let edition_1_with_old_syntax = format!("// edition: 1\n{edition_02}");
    let edition_1 = "// edition: 1\nfn i32 answer() = 42\n";
    let edition_1_without_header = "fn i32 answer() = 42\n";

    let mut client = Client::start();
    initialize(&mut client, 1, "[]");
    client.notification("initialized", "{}");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"languageId\":\"nexul\",\"version\":1,\"text\":{}}}}}",
            quote(&document_uri),
            quote(edition_02)
        ),
    );
    let initial = client.diagnostics(&document_uri, 1);
    assert!(initial.contains("\"diagnostics\":[]"), "{initial}");

    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":2}},\"contentChanges\":[{{\"text\":{}}}]}}",
            quote(&document_uri),
            quote(&edition_1_with_old_syntax)
        ),
    );
    let added_header = client.diagnostics(&document_uri, 2);
    assert!(
        !added_header.contains("\"diagnostics\":[]"),
        "adding an edition-1 header did not switch editions: {added_header}"
    );

    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":3}},\"contentChanges\":[{{\"text\":{}}}]}}",
            quote(&document_uri),
            quote(edition_1)
        ),
    );
    let fixed_edition_1 = client.diagnostics(&document_uri, 3);
    assert!(
        fixed_edition_1.contains("\"diagnostics\":[]"),
        "{fixed_edition_1}"
    );

    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":4}},\"contentChanges\":[{{\"text\":{}}}]}}",
            quote(&document_uri),
            quote(edition_1_without_header)
        ),
    );
    let removed_header = client.diagnostics(&document_uri, 4);
    assert!(
        !removed_header.contains("\"diagnostics\":[]"),
        "removing the edition-1 header did not restore edition 0.2: {removed_header}"
    );

    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":5}},\"contentChanges\":[{{\"text\":{}}}]}}",
            quote(&document_uri),
            quote(edition_02)
        ),
    );
    let fixed_edition_02 = client.diagnostics(&document_uri, 5);
    assert!(
        fixed_edition_02.contains("\"diagnostics\":[]"),
        "{fixed_edition_02}"
    );
    assert_eq!(client.exit(true), 0);
}

#[test]
fn lsp_did_change_applies_utf8_and_utf16_positions_across_crlf_and_emoji() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let scratch = Scratch::new();
    let path = scratch.path().join("unicode.nx");
    let uri = uri(&path);
    let original = "// café 😀\r\nfn main(){}\r\n";

    let mut client = Client::start();
    initialize(&mut client, 1, "[\"utf-16\"]");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":1,\"text\":{}}}}}",
            quote(&uri),
            quote(original)
        ),
    );
    // In UTF-16, the emoji occupies two code units; replace its full span without splitting its surrogate pair.
    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":2}},\"contentChanges\":[{{\"range\":{{\"start\":{{\"line\":0,\"character\":8}},\"end\":{{\"line\":0,\"character\":10}}}},\"text\":\"x\"}}]}}",
            quote(&uri)
        ),
    );
    client.request(
        2,
        "textDocument/formatting",
        &format!("{{\"textDocument\":{{\"uri\":{}}}}}", quote(&uri)),
    );
    let formatting = client.response(2);
    assert!(formatting.contains("\"result\":"), "{formatting}");
    assert!(
        formatting.contains("café x"),
        "UTF-16 edit was not applied: {formatting}"
    );
    assert_eq!(client.exit(true), 0);

    let mut client = Client::start();
    initialize(&mut client, 1, "[\"utf-8\"]");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":1,\"text\":{}}}}}",
            quote(&uri),
            quote(original)
        ),
    );
    // In UTF-8, the same emoji occupies four bytes.
    client.notification(
        "textDocument/didChange",
        &format!(
        "{{\"textDocument\":{{\"uri\":{},\"version\":2}},\"contentChanges\":[{{\"range\":{{\"start\":{{\"line\":0,\"character\":9}},\"end\":{{\"line\":0,\"character\":13}}}},\"text\":\"x\"}}]}}",
            quote(&uri)
        ),
    );
    client.request(
        2,
        "textDocument/formatting",
        &format!("{{\"textDocument\":{{\"uri\":{}}}}}", quote(&uri)),
    );
    let formatting = client.response(2);
    assert!(formatting.contains("\"result\":"), "{formatting}");
    assert!(
        formatting.contains("café x"),
        "UTF-8 edit was not applied: {formatting}"
    );
    assert_eq!(client.exit(true), 0);
}

#[test]
fn lsp_reports_missing_cached_dependency_on_manifest_without_fetching() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.path().join("src")).unwrap();
    std::fs::write(
        scratch.path().join("nexul.toml"),
        "[package]\nname = \"missing-dependency\"\nversion = \"0.1.0\"\n\n[dependencies]\nremote = { git = \"https://example.invalid/remote.git\", tag = \"v1\" }\n",
    )
    .unwrap();
    let main_path = scratch.path().join("src/main.nx");
    let main = "fn main() {}\n";
    std::fs::write(&main_path, main).unwrap();
    let main_uri = uri(&main_path);
    let manifest_uri = uri(&scratch.path().join("nexul.toml"));

    let mut client = Client::start();
    initialize(&mut client, 1, "[]");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":1,\"text\":{}}}}}",
            quote(&main_uri),
            quote(main)
        ),
    );
    let diagnostic = client.diagnostics_without_version(&manifest_uri);
    assert!(diagnostic.contains("dependency `remote`"), "{diagnostic}");
    assert!(!scratch.path().join("nexul.lock").exists());
    assert_eq!(client.exit(true), 0);
}

#[cfg(unix)]
#[test]
fn diagnostics_keep_the_client_uri_through_a_directory_alias() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let scratch = Scratch::new();
    let project = scratch.path().join("real");
    std::fs::create_dir_all(project.join("src/tools")).unwrap();
    let main = "import tools.calc\nfn main() { let answer: i32 = calc.value(); println(answer) }\n";
    let calc = "pub fn value() -> i32 { return 42 }\n";
    std::fs::write(project.join("nexul.toml"), "[package]\nname = \"alias\"\nversion = \"0.1.0\"\n").unwrap();
    std::fs::write(project.join("src/main.nx"), main).unwrap();
    std::fs::write(project.join("src/tools/calc.nx"), calc).unwrap();
    let alias = scratch.path().join("alias");
    std::os::unix::fs::symlink(&project, &alias).unwrap();
    let main_uri = uri(&alias.join("src/main.nx"));
    let calc_uri = uri(&alias.join("src/tools/calc.nx"));
    let mut client = Client::start();
    initialize(&mut client, 1, "[]");
    for (uri, text) in [(&main_uri, main), (&calc_uri, calc)] {
        client.notification("textDocument/didOpen", &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":1,\"text\":{}}}}}", quote(uri), quote(text)));
    }
    assert!(client.diagnostics(&main_uri, 1).contains("\"diagnostics\":[]"));
    assert!(client.diagnostics(&calc_uri, 1).contains("\"diagnostics\":[]"));
    client.notification("textDocument/didChange", &format!(
        "{{\"textDocument\":{{\"uri\":{},\"version\":2}},\"contentChanges\":[{{\"text\":{}}}]}}",
        quote(&calc_uri), quote("pub fn value() -> String { return \"wrong\" }\n")));
    let diagnostics = client.diagnostics(&main_uri, 1);
    assert!(diagnostics.contains("\"diagnostics\":[{"), "{diagnostics}");
    assert_eq!(client.exit(true), 0);

    std::fs::write(project.join("nexul.toml"),
        "[package]\nname = \"alias\"\nversion = \"0.1.0\"\n\n[dependencies]\nremote = { git = \"https://example.invalid/remote.git\", tag = \"v1\" }\n").unwrap();
    let mut client = Client::start();
    initialize(&mut client, 1, "[]");
    client.notification("textDocument/didOpen", &format!(
        "{{\"textDocument\":{{\"uri\":{},\"version\":1,\"text\":{}}}}}", quote(&main_uri), quote(main)));
    let diagnostics = client.diagnostics_without_version(&uri(&alias.join("nexul.toml")));
    assert!(diagnostics.contains("dependency `remote`"), "{diagnostics}");
    assert!(!project.join("nexul.lock").exists());
    assert_eq!(client.exit(true), 0);
}

#[test]
fn lsp_hot_requests_and_diagnostics_meet_the_latency_targets() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let path = repo.join("examples/calculatrice.nx");
    let text = std::fs::read_to_string(&path).unwrap();
    let uri = uri(&path);
    let hover_offset = text.find("Token").unwrap();
    let hover_line = text[..hover_offset].matches('\n').count();
    let hover_line_start = text[..hover_offset]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let hover_character = text[hover_line_start..hover_offset].len();
    let completion_offset = text.find("text.chars()").unwrap() + "text.".len();
    let completion_line = text[..completion_offset].matches('\n').count();
    let completion_line_start = text[..completion_offset]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let completion_character = text[completion_line_start..completion_offset].len();
    let hover_params = format!(
        "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}}}}",
        quote(&uri),
        hover_line,
        hover_character
    );
    let completion_params = format!(
        "{{\"textDocument\":{{\"uri\":{}}},\"position\":{{\"line\":{},\"character\":{}}}}}",
        quote(&uri),
        completion_line,
        completion_character
    );
    let text_document_params = format!("{{\"textDocument\":{{\"uri\":{}}}}}", quote(&uri));

    let mut client = Client::start();
    initialize(&mut client, 1, "[\"utf-8\"]");
    client.notification(
        "textDocument/didOpen",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":1,\"text\":{}}}}}",
            quote(&uri),
            quote(&text)
        ),
    );
    let (warmup_response, warmup) = client.timed_request(2, "textDocument/hover", &hover_params);
    assert!(!warmup_response.contains("\"error\":"), "{warmup_response}");
    let initial_diagnostics = client.diagnostics(&uri, 1);
    assert!(initial_diagnostics.contains("publishDiagnostics"));

    let mut id = 3;
    for (method, params) in [
        ("textDocument/hover", hover_params.as_str()),
        ("textDocument/definition", hover_params.as_str()),
        ("textDocument/completion", completion_params.as_str()),
    ] {
        let mut method_samples = Vec::new();
        for _ in 0..20 {
            let (response, elapsed) = client.timed_request(id, method, params);
            assert!(!response.contains("\"error\":"), "{method}: {response}");
            method_samples.push(elapsed);
            id += 1;
        }
        method_samples.sort();
        let p95 = method_samples[(method_samples.len() * 95).div_ceil(100) - 1];
        eprintln!("nxc lsp {method}: p95={:.2?}", p95);
        if !cfg!(debug_assertions) {
            assert!(p95 < Duration::from_millis(50), "{method} p95 was {p95:?}");
        }
    }

    let changed = format!("// revision 2\n{}", text);
    let started = Instant::now();
    client.notification(
        "textDocument/didChange",
        &format!(
            "{{\"textDocument\":{{\"uri\":{},\"version\":2}},\"contentChanges\":[{{\"text\":{}}}]}}",
            quote(&uri), quote(&changed)
        ),
    );
    client.diagnostics(&uri, 2);
    let diagnostics_latency = started.elapsed();
    eprintln!("nxc lsp diagnostics latency: {diagnostics_latency:.2?}; warmup {warmup:.2?}");
    if !cfg!(debug_assertions) {
        assert!(
            diagnostics_latency < Duration::from_millis(300),
            "diagnostics took {diagnostics_latency:?}"
        );
    }

    #[cfg(windows)]
    let mut working_sets = process_working_set(client.child.id())
        .into_iter()
        .collect::<Vec<_>>();
    for revision in 3..=1002 {
        let changed = format!("// revision {revision}\n{}", text);
        client.notification(
            "textDocument/didChange",
            &format!(
                "{{\"textDocument\":{{\"uri\":{},\"version\":{}}},\"contentChanges\":[{{\"text\":{}}}]}}",
                quote(&uri), revision, quote(&changed)
            ),
        );
        #[cfg(windows)]
        if revision % 250 == 2 {
            let (response, _) =
                client.timed_request(id, "textDocument/documentSymbol", &text_document_params);
            assert!(!response.contains("\"error\":"), "{response}");
            id += 1;
            if let Some(working_set) = process_working_set(client.child.id()) {
                working_sets.push(working_set);
            }
        }
    }
    let final_diagnostics = client.diagnostics(&uri, 1002);
    assert!(final_diagnostics.contains("publishDiagnostics"));
    #[cfg(windows)]
    if let Some(working_set) = process_working_set(client.child.id()) {
        working_sets.push(working_set);
    }
    #[cfg(windows)]
    if let (Some(minimum), Some(maximum)) = (working_sets.iter().min(), working_sets.iter().max()) {
        let spread = *maximum - *minimum;
        eprintln!(
            "nxc lsp working-set range after 1,000 changes: {:.2} MiB",
            spread as f64 / (1024.0 * 1024.0)
        );
        assert!(
            spread < 32 * 1024 * 1024,
            "working set grew by {spread} bytes during 1,000 edits"
        );
    }
    let (response, _) =
        client.timed_request(id, "textDocument/documentSymbol", &text_document_params);
    assert!(!response.contains("\"error\":"), "{response}");
    assert_eq!(client.exit(true), 0);
}
