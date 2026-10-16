use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};

fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.trim_end_matches(['\r', '\n']).split_once(':') {
            if name.eq_ignore_ascii_case("Content-Length") {
                length = value.trim().parse::<usize>().ok();
            }
        }
    }
    let length = length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(io::Error::other)
}

fn send(writer: &mut impl Write, value: Value) -> io::Result<()> {
    let body = serde_json::to_vec(&value).map_err(io::Error::other)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let exit_after_launch = std::env::args().any(|arg| arg == "--exit-after-launch");
    let mut pending_start = None;
    while let Some(message) = read_frame(&mut reader)? {
        let seq = message["seq"].as_i64().unwrap_or(0);
        let command = message["command"].as_str().unwrap_or_default();
        let mut body = json!({});
        match command {
            "launch" | "attach" => {
                pending_start = Some((seq, command.to_owned()));
                send(
                    &mut writer,
                    json!({ "seq": 500, "type": "event", "event": "initialized", "body": {} }),
                )?;
                if exit_after_launch {
                    break;
                }
                continue;
            }
            "initialize" => {
                body = json!({ "adapterID": message["arguments"]["adapterID"], "supportsConditionalBreakpoints": true, "supportsConfigurationDoneRequest": true });
            }
            "setBreakpoints" => {
                body = json!({ "breakpoints": message["arguments"]["breakpoints"] });
            }
            "threads" => {
                body = json!({ "threads": [{ "id": 7, "name": "fake-thread" }] });
            }
            "stackTrace" => {
                body = json!({ "stackFrames": [{ "id": 4, "name": "main", "line": 9 }], "totalFrames": 1 });
            }
            "scopes" => {
                body = json!({ "scopes": [{ "name": "Locals", "variablesReference": 11, "expensive": false }] });
            }
            "variables" => {
                body = json!({ "variables": [{ "name": "answer", "value": "42", "variablesReference": 0 }] });
            }
            "evaluate" => {
                body = json!({ "result": "42", "variablesReference": 0 });
            }
            _ => {}
        }
        send(
            &mut writer,
            json!({ "seq": seq + 100, "type": "response", "request_seq": seq, "command": command, "success": true, "body": body }),
        )?;
        if command == "configurationDone" {
            if let Some((start_seq, start_command)) = pending_start.take() {
                send(
                    &mut writer,
                    json!({ "seq": start_seq + 100, "type": "response", "request_seq": start_seq, "command": start_command, "success": true, "body": {} }),
                )?;
            }
            send(
                &mut writer,
                json!({ "seq": 501, "type": "event", "event": "stopped", "body": { "reason": "breakpoint", "threadId": 7, "allThreadsStopped": true } }),
            )?;
        } else if command == "disconnect" {
            break;
        }
    }
    Ok(())
}
