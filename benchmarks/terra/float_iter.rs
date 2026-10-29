fn main() {
    let mut x = 1.0f64;
    let mut total = 0.0f64;
    for i in 0..10_000_000 {
        x = x * 0.5 + (i % 100) as f64 * 0.25;
        total += x;
    }
    println!("{}", total as u64);
}
