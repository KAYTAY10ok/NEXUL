fn main() {
    let n = 5_000_000usize;
    let mut text = String::with_capacity(n);
    for i in 0..n { text.push((b'a' + (i % 26) as u8) as char); }
    let mut sum = 0u64;
    for byte in text.bytes() { sum += byte as u64; }
    println!("{sum}");
}
