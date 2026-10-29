fn main() {
    let n: usize = 4_000_000;
    let mut values: Vec<u64> = Vec::with_capacity(n);
    for i in 0..n { values.push((i % 1000) as u64); }
    for _ in 0..3 {
        for value in &mut values { *value = *value * 3 + 7; }
    }
    let mut sum = 0u64;
    for value in values { sum += value; }
    println!("{sum}");
}
