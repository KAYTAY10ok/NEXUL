use std::fs;

fn main() -> std::io::Result<()> {
    let mut count: u64 = 0;
    let mut checksum: u64 = 0;
    for _ in 0..400 {
        let bytes = fs::read("lib/std/io.nx")?;
        let mut in_word = false;
        for b in bytes {
            let word = (65..=90).contains(&b) || (97..=122).contains(&b) || (48..=57).contains(&b) || b == 95;
            if word && !in_word { count += 1; }
            in_word = word;
            checksum += b as u64;
        }
    }
    println!("tokens={count} checksum={checksum}");
    Ok(())
}
