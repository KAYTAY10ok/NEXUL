//! The bound of the stack of the real-time tasks (ADR 0018 §6), on Cortex-M4 under QEMU:
//!
//! - `bound_covers_the_observed_maximum`: for each level of optimization, the bound that
//!   `nxc` computes on the executable (`--stack-report`) is at least the most stack the
//!   tasks used (`rt.stack_used()`, measured by painting the stack).
//! - `a_bound_above_the_reserve_is_refused`: E0607, and no executable is left.
//!
//! Run when `NEXUL_TEST_TARGETS` names `thumbv7em-none` (QEMU needed), as in the CI.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn enabled() -> bool {
    std::env::var("NEXUL_TEST_TARGETS").map_or(false, |t| t.split(',').any(|x| x.trim() == "thumbv7em-none"))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nxc-rt-stack-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The program of `tests/run/rt_tasks.nx`, which also prints the stack it used.
fn measured_program() -> String {
    let src = std::fs::read_to_string(root().join("tests/run/rt_tasks.nx")).unwrap();
    let marker = "    println(\"haut {e.haut}";
    assert!(src.contains(marker));
    src.replacen(marker, &format!("    println(\"pile {{rt.stack_used()}}\")\n{}", marker), 1)
}

#[test]
fn bound_covers_the_observed_maximum() {
    if !enabled() {
        return;
    }
    let dir = scratch("bound");
    let nx = dir.join("m.nx");
    std::fs::write(&nx, measured_program()).unwrap();
    for level in ["-O0", "-O2", "-Os"] {
        let exe = dir.join(format!("m{}.elf", level));
        let out = Command::new(env!("CARGO_BIN_EXE_nxc"))
            .args(["build", level, "--target", "thumbv7em-none", "--stack-report", "-o"])
            .arg(&exe)
            .arg(&nx)
            .env("NEXUL_NO_CACHE", "1")
            .output()
            .unwrap();
        let report = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(out.status.success(), "{}: nxc build failed:\n{}", level, report);
        let bound: u64 = report
            .lines()
            .find_map(|l| l.strip_prefix("stack of the real-time tasks (").and_then(|r| r.split(' ').next()).and_then(|n| n.parse().ok()))
            .unwrap_or_else(|| panic!("{}: no bound in the report:\n{}", level, report));
        let run = Command::new("timeout")
            .args(["120", "qemu-system-arm", "-machine", "mps2-an386", "-icount", "shift=5", "-nographic", "-monitor", "none", "-serial", "none", "-semihosting-config", "enable=on,target=native", "-kernel"])
            .arg(&exe)
            .output()
            .expect("run QEMU");
        let stdout = String::from_utf8_lossy(&run.stdout).to_string();
        let used: u64 = stdout
            .lines()
            .find_map(|l| l.strip_prefix("pile ").and_then(|n| n.trim().parse().ok()))
            .unwrap_or_else(|| panic!("{}: no measure in the output:\n{}{}", level, stdout, String::from_utf8_lossy(&run.stderr)));
        assert!(used > 0, "{}: nothing measured", level);
        assert!(used <= bound, "{}: the tasks used {} bytes of stack, more than the bound of {} bytes\n{}", level, used, bound, report);
        eprintln!("{}: used {} <= bound {}", level, used, bound);
    }
}

#[test]
fn a_bound_above_the_reserve_is_refused() {
    if !enabled() {
        return;
    }
    let dir = scratch("refused");
    let nx = dir.join("big.nx");
    std::fs::write(
        &nx,
        r#"// edition: 1
import std.rt
import std.rt.{ms}

@resource
static var u64 somme = 0

// 4 KiB of local array: more than the reserve of 1 KiB.
fn lourde(u64 somme!) every ms(1) with noalloc, nonblocking {
    var [u64; 512] t = [0; 512]
    for i in 0..512 {
        t[i] = somme + i as u64
    }
    somme = t[(somme % 512) as usize]
    rt.stop()
}

@clock(25_000_000)
@stack(1024)
fn main() {
    rt.run()
    println("{somme}")
}
"#,
    )
    .unwrap();
    let exe = dir.join("big.elf");
    let out = Command::new(env!("CARGO_BIN_EXE_nxc"))
        .args(["build", "--target", "thumbv7em-none", "-o"])
        .arg(&exe)
        .arg(&nx)
        .env("NEXUL_NO_CACHE", "1")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "the build should fail");
    assert!(err.contains("error[E0607]") && err.contains("more than the reserve of 1024 bytes"), "{}", err);
    assert!(!exe.exists(), "an executable whose stack is not bounded is left");
}
