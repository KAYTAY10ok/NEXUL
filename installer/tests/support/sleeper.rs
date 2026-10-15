//! Stands for a program of NEXUL (Studio, nxc) in the tests of the installer: sleeps for
//! the number of seconds given (60 by default).

fn main() {
    let secs = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    std::thread::sleep(std::time::Duration::from_secs(secs));
}
