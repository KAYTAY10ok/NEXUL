use std::fs::{self, OpenOptions};
use std::io::{Read, Write};

fn main() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!("nexul-bench-{}", std::process::id()));
    fs::create_dir(&dir)?;
    let path = dir.join("data");
    let bytes: Vec<u8> = (0..65536).map(|i| (i % 251) as u8).collect();
    let mut sum: u64 = 0;
    for _ in 0..128 {
        let mut f = OpenOptions::new().write(true).create(true).truncate(true).open(&path)?;
        f.write_all(&bytes)?;
        drop(f);
        let mut data = Vec::new();
        fs::File::open(&path)?.read_to_end(&mut data)?;
        for b in data { sum += b as u64; }
    }
    fs::remove_file(path)?;
    fs::remove_dir(dir)?;
    println!("checksum={sum}");
    Ok(())
}
