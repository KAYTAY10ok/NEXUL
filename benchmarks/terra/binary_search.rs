fn main() {
    let n = 65_536usize;
    let count = 1_000_000 + std::env::args().count() - 1;
    let values: Vec<u64> = (0..n).map(|i| (i * 3) as u64).collect();
    let (mut x, mut checksum) = (7u64, 0u64);
    for _ in 0..count {
        x = (x * 48_271 + 17) % 2_147_483_647;
        let key = x % 196_608;
        let (mut lo, mut hi) = (0, n);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if values[mid] < key { lo = mid + 1; } else { hi = mid; }
        }
        checksum += lo as u64;
    }
    println!("{checksum}");
}
