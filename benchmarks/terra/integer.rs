fn main() {
    let count = 20_000_000 + std::env::args().count() - 1;
    let (mut x, mut sum) = (7u64, 0u64);
    for _ in 0..count {
        x = (x * 48_271 + 17) % 2_147_483_647;
        sum += x % 1009;
    }
    println!("{sum}");
}
