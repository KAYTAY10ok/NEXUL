use nexul_studio_dap::{AdapterCommand, Breakpoint, DapClient, DapError};
use serde_json::json;
use std::path::Path;
use std::thread;
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
    let mut launch = client.begin_launch(json!({ "program": "fake" })).unwrap();
    assert!(next_event(&client, "initialized"));
    assert!(launch.try_wait().unwrap().is_none());
    let breakpoints = [Breakpoint::new(9).condition("answer == 42")];
    assert!(
        client
            .set_breakpoints(Path::new("main.nx"), &breakpoints)
            .unwrap()
            .success
    );
    assert!(client.configuration_done().unwrap().success);
    let started = std::time::Instant::now();
    let launch_response = loop {
        if let Some(response) = launch.try_wait().unwrap() {
            break response;
        }
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "launch response timed out"
        );
        thread::sleep(Duration::from_millis(5));
    };
    assert!(launch_response.success);
    assert!(launch.try_wait().unwrap().is_none());
    assert!(launch.wait(Duration::from_secs(1)).unwrap().success);
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

#[test]
fn attach_can_be_configured_before_its_response() {
    let adapter = AdapterCommand::new(env!("CARGO_BIN_EXE_fake-dap-adapter"));
    let mut client = DapClient::spawn(&adapter).unwrap();
    assert!(client.initialize("test", "test").unwrap().success);
    let attach = client.begin_attach(json!({ "pid": 7 })).unwrap();
    assert!(next_event(&client, "initialized"));
    assert!(client.configuration_done().unwrap().success);
    let response = attach.wait(Duration::from_secs(1)).unwrap();
    assert!(response.success);
    assert_eq!(response.command, "attach");
    client.shutdown(Duration::from_secs(1)).unwrap();
}

#[test]
fn late_response_after_timeout_or_abandon_does_not_break_routing() {
    let adapter = AdapterCommand::new(env!("CARGO_BIN_EXE_fake-dap-adapter"));
    let mut client = DapClient::spawn(&adapter).unwrap();
    assert!(client.initialize("test", "test").unwrap().success);
    let launch = client.begin_launch(json!({ "program": "fake" })).unwrap();
    assert!(next_event(&client, "initialized"));
    assert!(
        matches!(launch.wait(Duration::from_millis(20)), Err(DapError::Timeout(command)) if command == "launch")
    );
    assert!(client.configuration_done().unwrap().success);
    drop(client.begin_launch(json!({ "program": "fake" })).unwrap());
    assert!(next_event(&client, "initialized"));
    assert!(client.configuration_done().unwrap().success);
    assert_eq!(client.threads().unwrap().body["threads"][0]["id"], 7);
    client.shutdown(Duration::from_secs(1)).unwrap();
}

#[test]
fn pending_launch_reports_adapter_exit() {
    let adapter =
        AdapterCommand::new(env!("CARGO_BIN_EXE_fake-dap-adapter")).arg("--exit-after-launch");
    let mut client = DapClient::spawn(&adapter).unwrap();
    assert!(client.initialize("test", "test").unwrap().success);
    let launch = client.begin_launch(json!({ "program": "fake" })).unwrap();
    assert!(next_event(&client, "initialized"));
    assert!(next_event(&client, "adapterExited"));
    assert!(matches!(
        launch.wait(Duration::from_secs(2)),
        Err(DapError::AdapterExited)
    ));
    assert!(matches!(
        client.request_timeout("threads", json!({}), Duration::from_secs(2)),
        Err(DapError::AdapterExited)
    ));
    client.shutdown(Duration::from_secs(1)).unwrap();
}
