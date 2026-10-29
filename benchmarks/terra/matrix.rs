fn main() {
    let n = 64usize;
    let rounds = 300 + std::env::args().count() - 1;
    let a: Vec<u64> = (0..n*n).map(|i| (i % 17) as u64).collect();
    let b: Vec<u64> = (0..n*n).map(|i| (i % 13) as u64).collect();
    let mut c = vec![0u64; n*n];
    for _ in 0..rounds {
        for i in 0..n {
            for j in 0..n {
                let mut value = 0;
                for k in 0..n { value += a[i*n+k] * b[k*n+j]; }
                c[i*n+j] += value;
            }
        }
    }
    println!("{}", c.iter().sum::<u64>());
}
