use nexul_studio_dap::{
    build_nexul_program, find_nxc, lldb_launch_arguments, AdapterCommand, AdapterKind,
    AdapterRegistry, Breakpoint, DapClient, DapEvent,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn next_matching(client: &DapClient, event_names: &[&str], timeout: Duration) -> Option<DapEvent> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match client.recv_event_timeout(Duration::from_millis(100)) {
            Ok(event) if event_names.contains(&event.event.as_str()) => return Some(event),
            Ok(_) => {}
            Err(_) => {}
        }
    }
    None
}

#[test]
fn lldb_dap_debugs_a_nexul_program_with_breakpoint_variables_and_step() {
    let Some(nxc) = find_nxc() else {
        eprintln!("SKIP: set NEXUL_NXC or put nxc in PATH to run the NEXUL/LLDB integration test");
        return;
    };
    let registry = AdapterRegistry::discover(Some(&nxc));
    let Some(adapter) = registry.get(AdapterKind::LldbDap).cloned() else {
        eprintln!("SKIP: lldb-dap was not found; install it with LLVM or NEXUL");
        return;
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexul-dap-test-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("main.nx");
    let output = root.join(if cfg!(windows) {
        "debug-program.exe"
    } else {
        "debug-program"
    });
    fs::write(
        &source,
        "fn main() {\n    let answer = 42\n    println(\"answer = {answer}\")\n}\n",
    )
    .unwrap();
    let program = build_nexul_program(&nxc, &source, &output).unwrap();
    if cfg!(windows) {
        assert!(
            program.pdb.as_ref().is_some_and(|pdb| pdb.is_file()),
            "debug build should produce a PDB"
        );
    }

    let mut client = DapClient::spawn(&adapter).unwrap();
    assert!(
        client
            .initialize("nexul-test", "NEXUL Studio DAP integration")
            .unwrap()
            .success
    );
    assert!(
        client
            .launch(lldb_launch_arguments(
                &program.executable,
                &root,
                &[],
                false
            ))
            .unwrap()
            .success
    );
    assert!(
        next_matching(&client, &["initialized"], Duration::from_secs(10)).is_some(),
        "adapter did not initialize"
    );
    assert!(
        client
            .set_breakpoints(&source, &[Breakpoint::new(3)])
            .unwrap()
            .success
    );
    assert!(client.configuration_done().unwrap().success);
    let stopped = next_matching(
        &client,
        &["stopped", "terminated", "exited"],
        Duration::from_secs(20),
    )
    .expect("program did not stop at its breakpoint");
    assert_eq!(stopped.event, "stopped");
    let thread_id = stopped.body["threadId"]
        .as_i64()
        .expect("stopped event has no threadId");
    let stack = client.stack_trace(thread_id, Some(0), Some(20)).unwrap();
    let frame = stack.body["stackFrames"][0]["id"]
        .as_i64()
        .expect("no stack frame");
    let scopes = client.scopes(frame).unwrap();
    let variables_reference = scopes.body["scopes"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|scope| scope["name"].as_str().unwrap_or_default().contains("Local"))
        })
        .and_then(|scope| scope["variablesReference"].as_i64())
        .expect("no local scope");
    let variables = client.variables(variables_reference).unwrap();
    assert!(
        variables.body["variables"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["name"] == "answer")),
        "local variable `answer` was not visible: {}",
        variables.body
    );
    assert!(client.next(thread_id).unwrap().success);
    let stepped = next_matching(
        &client,
        &["stopped", "terminated", "exited"],
        Duration::from_secs(10),
    )
    .expect("step did not stop the program");
    assert_eq!(stepped.event, "stopped");
    let _ = client.continue_execution(thread_id);
    let _ = next_matching(&client, &["exited", "terminated"], Duration::from_secs(15));
    let _ = client.disconnect(true);
    client.shutdown(Duration::from_secs(2)).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[allow(dead_code)]
fn _value_type_anchor(_: Value, _: PathBuf, _: AdapterCommand) {}
