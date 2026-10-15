use nexul_studio_dap::{AdapterCommand, Breakpoint, DapClient};
use serde_json::json;
use std::path::Path;
use std::time::Duration;

fn next_event(client: &DapClient, wanted: &str) -> bool {
    for _ in 0..10 {
        match client.recv_event_timeout(Duration::from_millis(250)) {
            Ok(event) if event.event == wanted => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

#[test]
fn fake_adapter_exercises_dap_lifecycle_requests_and_events() {
    let adapter = AdapterCommand::new(env!("CARGO_BIN_EXE_fake-dap-adapter"));
    let mut client = DapClient::spawn(&adapter).unwrap();
    let initialized = client.initialize("nexul-test", "NEXUL DAP test").unwrap();
    assert!(initialized.success);
    assert_eq!(initialized.body["adapterID"], "nexul");
    assert!(next_event(&client, "initialized"));
    assert!(client.launch(json!({ "program": "fake" })).unwrap().success);
    let breakpoints = [Breakpoint::new(9).condition("answer == 42")];
    assert!(
        client
            .set_breakpoints(Path::new("main.nx"), &breakpoints)
            .unwrap()
            .success
    );
    assert!(client.configuration_done().unwrap().success);
    assert!(next_event(&client, "stopped"));
    assert_eq!(client.threads().unwrap().body["threads"][0]["id"], 7);
    let frame = client.stack_trace(7, None, Some(5)).unwrap().body["stackFrames"][0]["id"]
        .as_i64()
        .unwrap();
    let scopes = client.scopes(frame).unwrap().body["scopes"].clone();
    assert_eq!(
        client
            .variables(scopes[0]["variablesReference"].as_i64().unwrap())
            .unwrap()
            .body["variables"][0]["name"],
        "answer"
    );
    assert_eq!(
        client.evaluate("answer", Some(frame), "repl").unwrap().body["result"],
        "42"
    );
    assert!(client.next(7).unwrap().success);
    assert!(client.step_in(7).unwrap().success);
    assert!(client.step_out(7).unwrap().success);
    assert!(client.pause(7).unwrap().success);
    assert!(client.continue_execution(7).unwrap().success);
    assert!(client.disconnect(true).unwrap().success);
    client.shutdown(Duration::from_secs(1)).unwrap();
}
