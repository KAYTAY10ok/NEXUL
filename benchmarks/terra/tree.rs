fn main() {
    let n = 50000usize;
    let mut total = 0u64;
    for _ in 0..4 {
        let mut keys = vec![0usize; n + 1];
        let mut left = vec![0usize; n + 1];
        let mut right = vec![0usize; n + 1];
        for i in 0..n {
            let key = (i * 48271 + 17) % n + 1;
            let node = i + 1;
            keys[node] = key;
            if node > 1 {
                let mut cur = 1;
                loop {
                    let child = if key < keys[cur] { &mut left[cur] } else { &mut right[cur] };
                    if *child == 0 { *child = node; break; }
                    cur = *child;
                }
            }
        }
        for i in 0..n {
            let key = (i * 48271 + 17) % n + 1;
            let mut cur = 1;
            while keys[cur] != key { cur = if key < keys[cur] { left[cur] } else { right[cur] }; }
            total += cur as u64;
        }
    }
    println!("checksum={total}");
}
