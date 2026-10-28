use std::hint::black_box;
use std::time::Instant;

fn vectors(n: usize) -> u64 {
    let input: Vec<u64> = (0..n).map(|i| (i % 1000 + 1) as u64).collect();
    let mut output = vec![0_u64; n];
    let mut i = 0;
    while i + 2 < n {
        let a = input[i];
        let b = input[i + 1];
        let c = input[i + 2];
        output[i] = 2 * a + 3 * b + 5 * c;
        output[i + 1] = 7 * a + 11 * b + 13 * c;
        output[i + 2] = 17 * a + 19 * b + 23 * c;
        i += 3;
    }
    while i < n {
        output[i] = input[i];
        i += 1;
    }
    output.iter().sum()
}

fn allocations(n: usize) -> u64 {
    let mut checksum = 0_u64;
    for i in 0..n as u64 {
        let block = black_box(Box::new([i, i + 3, i ^ 0x5a5a, i * 3 + 7]));
        checksum += block.iter().sum::<u64>();
        drop(block);
    }
    checksum
}

fn strings(n: usize) -> u64 {
    let mut text = String::with_capacity(n);
    for i in 0..n {
        let byte = b'a' + (i % 26) as u8;
        text.push(byte as char);
    }
    assert_eq!(text.len(), n);
    black_box(text.as_bytes())
        .iter()
        .map(|byte| u64::from(*byte))
        .sum()
}

fn calculate(mode: &str, n: usize) -> u64 {
    match mode {
        "vectors" => vectors(n),
        "allocations" => allocations(n),
        "strings" => strings(n),
        _ => panic!("unknown workload: {mode}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: dynamic <vectors|allocations|strings> <iterations>");
        std::process::exit(2);
    }
    let n: usize = args[2].parse().expect("iterations must be a positive integer");
    if n == 0 {
        eprintln!("iterations must be a positive integer");
        std::process::exit(2);
    }
    let warm_n = n.min(10_000);
    let mut warm_checksum = 0_u64;
    for _ in 0..10 {
        warm_checksum = warm_checksum.wrapping_add(calculate(&args[1], warm_n));
    }
    black_box(warm_checksum);

    let start = Instant::now();
    let checksum = black_box(calculate(&args[1], black_box(n)));
    let elapsed_ns = start.elapsed().as_nanos();
    println!("elapsed_ns={elapsed_ns}");
    println!("checksum={checksum}");
    println!("length={n}");
    println!("warmup_checksum={warm_checksum}");
}
