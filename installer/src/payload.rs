//! The data of the installation, appended to the installer executable:
//!
//! ```text
//! [installer program][compressed chunks][manifest][trailer: "NXSETUP1", start, manifest offset, manifest length]
//! ```
//!
//! The manifest is text, one record per line, fields separated by tabs:
//! `component id required default size name description`, then for each of its files
//! `file component path size sha256` followed by `chunk offset compressed-size size`.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const MAGIC: &[u8; 8] = b"NXSETUP1";
/// Files are compressed by pieces of this size (progress, memory).
pub const CHUNK: usize = 16 << 20;

#[derive(Clone, Debug, Default)]
pub struct Component {
    pub id: String,
    pub required: bool,
    pub default: bool,
    pub size: u64,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub offset: u64,
    pub clen: u64,
    pub len: u64,
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub component: String,
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub chunks: Vec<Chunk>,
}

#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub version: String,
    pub components: Vec<Component>,
    pub files: Vec<FileEntry>,
}

impl Manifest {
    pub fn to_text(&self) -> String {
        let mut s = format!("NEXUL-SETUP\t1\nversion\t{}\n", self.version);
        for c in &self.components {
            s.push_str(&format!("component\t{}\t{}\t{}\t{}\t{}\t{}\n", c.id, c.required as u8, c.default as u8, c.size, c.name, c.description));
        }
        for f in &self.files {
            s.push_str(&format!("file\t{}\t{}\t{}\t{}\n", f.component, f.path, f.size, f.sha256));
            for k in &f.chunks {
                s.push_str(&format!("chunk\t{}\t{}\t{}\n", k.offset, k.clen, k.len));
            }
        }
        s
    }

    pub fn parse(text: &str) -> Result<Manifest, String> {
        let mut m = Manifest::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let n = |i: usize| -> u64 { f.get(i).and_then(|x| x.parse().ok()).unwrap_or(0) };
            match f[0] {
                "NEXUL-SETUP" => {}
                "version" => m.version = f.get(1).unwrap_or(&"").to_string(),
                "component" if f.len() >= 7 => m.components.push(Component {
                    id: f[1].into(),
                    required: f[2] == "1",
                    default: f[3] == "1",
                    size: n(4),
                    name: f[5].into(),
                    description: f[6].into(),
                }),
                "file" if f.len() >= 5 => m.files.push(FileEntry { component: f[1].into(), path: f[2].into(), size: n(3), sha256: f[4].into(), chunks: vec![] }),
                "chunk" if f.len() >= 4 => match m.files.last_mut() {
                    Some(file) => file.chunks.push(Chunk { offset: n(1), clen: n(2), len: n(3) }),
                    None => return Err("manifeste invalide".into()),
                },
                _ => return Err(format!("manifeste invalide : {}", line)),
            }
        }
        Ok(m)
    }
}

/// Reads the manifest at the end of an installer: (manifest, size of the program alone).
pub fn read(exe: &Path) -> Result<(Manifest, u64), String> {
    let mut f = std::fs::File::open(exe).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    if len < 32 {
        return Err("aucune donnée d'installation".into());
    }
    f.seek(SeekFrom::Start(len - 32)).map_err(|e| e.to_string())?;
    let mut t = [0u8; 32];
    f.read_exact(&mut t).map_err(|e| e.to_string())?;
    if &t[..8] != MAGIC {
        return Err("aucune donnée d'installation dans ce programme".into());
    }
    let start = u64::from_le_bytes(t[8..16].try_into().unwrap());
    let moff = u64::from_le_bytes(t[16..24].try_into().unwrap());
    let mlen = u64::from_le_bytes(t[24..32].try_into().unwrap());
    f.seek(SeekFrom::Start(moff)).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; mlen as usize];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    let m = Manifest::parse(&String::from_utf8_lossy(&buf))?;
    Ok((m, start))
}

/// The bytes of a chunk.
pub fn read_chunk(f: &mut std::fs::File, k: &Chunk) -> Result<Vec<u8>, String> {
    f.seek(SeekFrom::Start(k.offset)).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; k.clen as usize];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// SHA-256, to check every file written.

pub fn sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74,
        0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d,
        0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e,
        0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut tail = data[data.len() / 64 * 64..].to_vec();
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend_from_slice(&bit_len.to_be_bytes());
    let full = &data[..data.len() / 64 * 64];
    for chunk in full.chunks(64).chain(tail.chunks(64)) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            v = [t1.wrapping_add(s0.wrapping_add(maj)), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{:08x}", x)).collect()
}
