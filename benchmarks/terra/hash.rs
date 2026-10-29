fn main() {
    let (n, cap) = (100000usize, 262144usize);
    let mut total = 0u64;
    for round in 0..5 {
        let mut keys = vec![0usize; cap];
        let mut values = vec![0u64; cap];
        for i in 0..n {
            let key = (i * 48271 + 17) % n + 1;
            let mut slot = (key * 2654435761) % cap;
            while keys[slot] != 0 { slot = (slot + 1) % cap; }
            keys[slot] = key;
            values[slot] = (i + round) as u64;
        }
        for i in 0..n {
            let key = (i * 48271 + 17) % n + 1;
            let mut slot = (key * 2654435761) % cap;
            while keys[slot] != key { slot = (slot + 1) % cap; }
            total += values[slot];
        }
    }
    println!("checksum={total}");
}
