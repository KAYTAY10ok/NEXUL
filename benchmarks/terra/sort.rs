fn main() {
    let n: usize = 100000;
    let mut total: u64 = 0;
    for round in 0..4 {
        let mut data: Vec<u64> = (0..n).map(|i| ((i * 48271 + 17 + round) % n) as u64).collect();
        let mut tmp = vec![0u64; n];
        let mut width = 1;
        while width < n {
            let mut start = 0;
            while start < n {
                let mid = (start + width).min(n);
                let end = (start + 2 * width).min(n);
                let (mut a, mut b) = (start, mid);
                for out in start..end {
                    if b == end || (a < mid && data[a] <= data[b]) { tmp[out] = data[a]; a += 1; }
                    else { tmp[out] = data[b]; b += 1; }
                }
                start = end;
            }
            data.copy_from_slice(&tmp);
            width *= 2;
        }
        for i in 0..n { total += data[i] * (i % 17 + 1) as u64; }
    }
    println!("checksum={total}");
}
