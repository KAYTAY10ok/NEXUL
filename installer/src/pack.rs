//! `nexul-pack <installer program> <staging folder> <output.exe>`: builds the installer
//! of NEXUL. The staging folder holds `components.txt` (one line per component:
//! `id<TAB>required<TAB>default<TAB>name<TAB>description`) and one folder per component
//! whose content is copied to the installation folder.

#[allow(dead_code)]
mod compress;
#[allow(dead_code)]
mod payload;

use payload::{Chunk, Component, FileEntry, Manifest, CHUNK, MAGIC};
use std::io::Write;
use std::path::{Path, PathBuf};

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            walk(root, &p, out);
        } else {
            out.push(p.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("usage: nexul-pack <NEXUL-Setup.exe> <staging folder> <output.exe>");
        std::process::exit(2);
    }
    let (stub, staging, out) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]), PathBuf::from(&args[2]));
    let program = std::fs::read(&stub).expect("cannot read the installer program");
    let list = std::fs::read_to_string(staging.join("components.txt")).expect("components.txt missing");
    let mut manifest = Manifest { version: env!("CARGO_PKG_VERSION").to_string(), ..Default::default() };
    // (component, relative path, data) of every chunk to compress.
    let mut jobs: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    for line in list.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        let (id, required, default, name, desc) = (f[0], f[1] == "1", f[2] == "1", f[3], f[4]);
        let dir = staging.join(id);
        let mut files = Vec::new();
        walk(&dir, &dir, &mut files);
        let mut size = 0u64;
        for rel in files {
            let data = std::fs::read(dir.join(&rel)).unwrap();
            size += data.len() as u64;
            let fi = manifest.files.len();
            manifest.files.push(FileEntry {
                component: id.to_string(),
                path: rel.to_string_lossy().replace('\\', "/"),
                size: data.len() as u64,
                sha256: payload::sha256(&data),
                chunks: vec![],
            });
            if data.is_empty() {
                continue;
            }
            for (ci, piece) in data.chunks(CHUNK).enumerate() {
                jobs.push((fi, ci, piece.to_vec()));
            }
        }
        manifest.components.push(Component { id: id.into(), required, default, size, name: name.into(), description: desc.into() });
        println!("{:<10} {:>8.1} Mo", id, size as f64 / 1048576.0);
    }
    // Compression in parallel (LZMS is slow but compresses well).
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let total = jobs.len();
    let jobs = std::sync::Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>());
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| loop {
                let job = jobs.lock().unwrap().pop();
                let Some((order, (fi, ci, data))) = job else { break };
                let c = compress::compress(&data).expect("compression failed");
                let done = {
                    let mut r = results.lock().unwrap();
                    r.push((order, fi, ci, data.len(), c));
                    r.len()
                };
                print!("\rcompression {}/{}", done, total);
                let _ = std::io::stdout().flush();
            });
        }
    });
    println!();
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    let mut body = program.clone();
    let start = body.len() as u64;
    for (_, fi, _ci, len, c) in results {
        let offset = body.len() as u64;
        body.extend_from_slice(&c);
        manifest.files[fi].chunks.push(Chunk { offset, clen: c.len() as u64, len: len as u64 });
    }
    let text = manifest.to_text();
    let moff = body.len() as u64;
    body.extend_from_slice(text.as_bytes());
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(&start.to_le_bytes());
    body.extend_from_slice(&moff.to_le_bytes());
    body.extend_from_slice(&(text.len() as u64).to_le_bytes());
    std::fs::write(&out, &body).expect("cannot write the installer");
    let raw: u64 = manifest.components.iter().map(|c| c.size).sum();
    println!("{} : {:.1} Mo (contenu : {:.1} Mo)", out.display(), body.len() as f64 / 1048576.0, raw as f64 / 1048576.0);
}
