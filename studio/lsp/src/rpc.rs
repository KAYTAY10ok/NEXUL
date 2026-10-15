//! JSON-RPC framing of the Language Server Protocol on a byte stream (stdio): each
//! message is `Content-Length: N\r\n` (other headers ignored), an empty line, then `N`
//! bytes of UTF-8 JSON.

use crate::json::{Json, ParseError};
use std::io::{self, BufRead, Write};

/// Messages larger than this are refused (protection against a broken stream).
pub const MAX_MESSAGE: usize = 256 * 1024 * 1024;

/// Writes one message and flushes.
pub fn write_message(w: &mut impl Write, message: &Json) -> io::Result<()> {
    let body = message.to_string();
    let mut buf = Vec::with_capacity(body.len() + 32);
    buf.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    buf.extend_from_slice(body.as_bytes());
    w.write_all(&buf)?;
    w.flush()
}

/// Reads one message. `Ok(None)` at the end of the stream before a message starts.
/// A body that is not valid JSON gives `Ok(Some(Err(..)))`: the framing is intact and the
/// next message can be read. Broken framing (missing or invalid `Content-Length`, stream
/// ending inside a message) is an `io::Error`.
pub fn read_message(r: &mut impl BufRead) -> io::Result<Option<Result<Json, ParseError>>> {
    let mut length: Option<usize> = None;
    let mut line = Vec::new();
    let mut first = true;
    loop {
        line.clear();
        let n = r.read_until(b'\n', &mut line)?;
        if n == 0 {
            if first {
                return Ok(None);
            }
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "stream ended inside the headers of a message"));
        }
        first = false;
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            break;
        }
        if let Some((name, value)) = text.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                let v =
                    value.trim().parse::<usize>().map_err(|_| invalid(format!("invalid Content-Length: {value:?}")))?;
                length = Some(v);
            }
        } else {
            return Err(invalid(format!("invalid header line: {text:?}")));
        }
    }
    let length = length.ok_or_else(|| invalid("message without Content-Length".into()))?;
    if length > MAX_MESSAGE {
        return Err(invalid(format!("message of {length} bytes refused")));
    }
    let mut body = vec![0; length];
    r.read_exact(&mut body)?;
    Ok(Some(match String::from_utf8(body) {
        Ok(text) => Json::parse(&text),
        Err(e) => Err(ParseError { offset: e.utf8_error().valid_up_to(), message: "invalid UTF-8".into() }),
    }))
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn write_then_read() {
        let mut buf = Vec::new();
        let a = Json::object([("jsonrpc", Json::from("2.0")), ("method", Json::from("é😀"))]);
        write_message(&mut buf, &a).unwrap();
        write_message(&mut buf, &Json::from(vec![])).unwrap();
        let text = String::from_utf8(buf.clone()).unwrap();
        // The length counts bytes, not characters.
        assert!(text.starts_with("Content-Length: 35\r\n\r\n{"), "{text}");
        let mut r = Cursor::new(buf);
        assert_eq!(read_message(&mut r).unwrap().unwrap().unwrap(), a);
        assert_eq!(read_message(&mut r).unwrap().unwrap().unwrap(), Json::Array(vec![]));
        assert!(read_message(&mut r).unwrap().is_none());
    }

    #[test]
    fn headers() {
        // Other headers, any case, bare `\n` line ends.
        let raw = "content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH:  2\n\n{}";
        let mut r = Cursor::new(raw.as_bytes().to_vec());
        assert_eq!(read_message(&mut r).unwrap().unwrap().unwrap(), Json::Object(vec![]));
        for bad in
            ["Content-Length: x\r\n\r\n", "Content-Type: a\r\n\r\n{}", "garbage\r\n\r\n", "Content-Length: 5\r\n\r\n{}"]
        {
            assert!(read_message(&mut Cursor::new(bad.as_bytes().to_vec())).is_err(), "{bad:?}");
        }
        let huge = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE + 1);
        assert!(read_message(&mut Cursor::new(huge.into_bytes())).is_err());
    }

    #[test]
    fn malformed_body_keeps_the_framing() {
        let raw = b"Content-Length: 3\r\n\r\n{x}Content-Length: 2\r\n\r\n\xff\xfeContent-Length: 4\r\n\r\ntrue";
        let mut r = Cursor::new(raw.to_vec());
        assert!(read_message(&mut r).unwrap().unwrap().is_err());
        assert!(read_message(&mut r).unwrap().unwrap().is_err());
        assert_eq!(read_message(&mut r).unwrap().unwrap().unwrap(), Json::Bool(true));
    }
}
