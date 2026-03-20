//! `game.pak` binary asset archive format.
//!
//! ## Format (little-endian)
//!
//! ```text
//! [Header]           8 bytes magic + 4 bytes version + 4 bytes entry count
//! [TOC Entry] × N    Each: 2 bytes name_len + name bytes + 1 byte kind
//!                          + 8 bytes offset + 8 bytes size
//! [Blob data]        Concatenated raw asset bytes
//! ```
//!
//! All offsets are relative to the start of the file.

use std::io::{self, Read, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"AMIGOPAK";
const VERSION: u32 = 2;
/// V1 is still accepted for reading (backward compat).
const VERSION_V1: u32 = 1;

/// The kind of asset stored in the pak file.
///
/// Type tags use 0x01–0x07 per spec. The old 0-based values from V1 are
/// accepted during read and mapped to the new tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AssetKind {
    Sprite = 0x01,
    Audio = 0x02,
    Data = 0x03,
    Level = 0x04,
    Font = 0x05,
    AtlasImage = 0x06,
    AtlasManifest = 0x07,
}

impl AssetKind {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            // V2 type tags
            0x01 => Some(Self::Sprite),
            0x02 => Some(Self::Audio),
            0x03 => Some(Self::Data),
            0x04 => Some(Self::Level),
            0x05 => Some(Self::Font),
            0x06 => Some(Self::AtlasImage),
            0x07 => Some(Self::AtlasManifest),
            _ => None,
        }
    }

    /// Map old V1 kind byte (0-based) to the new type tag.
    fn from_v1(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Sprite),
            1 => Some(Self::Audio),
            2 => Some(Self::Data),
            3 => Some(Self::Level),
            4 => Some(Self::Font),
            5 => Some(Self::AtlasImage),
            6 => Some(Self::AtlasManifest),
            _ => None,
        }
    }
}

/// Per-entry flags (bitmask).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssetFlags(pub u8);

impl AssetFlags {
    /// No special flags.
    pub const NONE: Self = Self(0x00);
    /// Asset data is LZ4-compressed.
    pub const COMPRESSED: Self = Self(0x01);
    /// Asset data is encrypted (reserved for future use).
    pub const ENCRYPTED: Self = Self(0x02);

    pub fn is_compressed(self) -> bool {
        self.0 & Self::COMPRESSED.0 != 0
    }

    pub fn is_encrypted(self) -> bool {
        self.0 & Self::ENCRYPTED.0 != 0
    }
}

/// A single entry in the pak table of contents.
#[derive(Clone, Debug)]
pub struct PakEntry {
    pub name: String,
    pub kind: AssetKind,
    pub flags: AssetFlags,
    pub offset: u64,
    pub size: u64,
}

/// Builder for creating a pak file.
pub struct PakWriter {
    entries: Vec<(String, AssetKind, AssetFlags, Vec<u8>)>,
}

impl Default for PakWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PakWriter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an asset from raw bytes.
    pub fn add(&mut self, name: impl Into<String>, kind: AssetKind, data: Vec<u8>) {
        self.entries
            .push((name.into(), kind, AssetFlags::NONE, data));
    }

    /// Add an asset with explicit flags.
    pub fn add_with_flags(
        &mut self,
        name: impl Into<String>,
        kind: AssetKind,
        flags: AssetFlags,
        data: Vec<u8>,
    ) {
        self.entries.push((name.into(), kind, flags, data));
    }

    /// Add an asset from a file on disk.
    pub fn add_file(
        &mut self,
        name: impl Into<String>,
        kind: AssetKind,
        path: &Path,
    ) -> io::Result<()> {
        let data = std::fs::read(path)?;
        self.add(name, kind, data);
        Ok(())
    }

    /// Number of entries added so far.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Write the pak file to disk.
    ///
    /// V2 format:
    /// ```text
    /// [Header]      8 bytes magic + 4 bytes version + 4 bytes entry count
    /// [TOC] × N     2 bytes name_len + name + 1 byte kind + 1 byte flags
    ///               + 8 bytes offset + 8 bytes size
    /// [Blob data]   Concatenated asset bytes
    /// [SHA256]      32 bytes SHA-256 hash of everything before it
    /// ```
    pub fn write_to(&self, path: &Path) -> io::Result<u64> {
        let mut buf: Vec<u8> = Vec::new();

        // -- Header --
        buf.write_all(MAGIC)?;
        buf.write_all(&VERSION.to_le_bytes())?;
        buf.write_all(&(self.entries.len() as u32).to_le_bytes())?;

        // -- Compute TOC size so we know blob offsets --
        let header_size: u64 = 8 + 4 + 4;
        let mut toc_size: u64 = 0;
        for (name, _, _, _) in &self.entries {
            // 2 bytes name_len + name + 1 kind + 1 flags + 8 offset + 8 size
            toc_size += 2 + name.len() as u64 + 1 + 1 + 8 + 8;
        }

        let blob_start = header_size + toc_size;
        let mut current_offset = blob_start;

        // -- Write TOC --
        for (name, kind, flags, data) in &self.entries {
            let name_bytes = name.as_bytes();
            buf.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
            buf.write_all(name_bytes)?;
            buf.write_all(&[*kind as u8])?;
            buf.write_all(&[flags.0])?;
            buf.write_all(&current_offset.to_le_bytes())?;
            buf.write_all(&(data.len() as u64).to_le_bytes())?;
            current_offset += data.len() as u64;
        }

        // -- Write blob data --
        for (_, _, _, data) in &self.entries {
            buf.write_all(data)?;
        }

        // -- Append SHA-256 integrity hash --
        let hash = sha256_hash(&buf);
        buf.write_all(&hash)?;

        let total_size = buf.len() as u64;
        std::fs::write(path, &buf)?;
        Ok(total_size)
    }
}

/// Compute a SHA-256 hash (minimal implementation, no external dep).
///
/// Uses the standard algorithm from FIPS 180-4.
fn sha256_hash(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Pre-processing: pad message
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit block
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = [0u8; 32];
    for (i, &val) in h.iter().enumerate() {
        result[4 * i..4 * i + 4].copy_from_slice(&val.to_be_bytes());
    }
    result
}

/// Verify the SHA-256 integrity hash of a pak file's raw bytes.
/// Returns `true` if the trailing 32 bytes match the hash of the preceding data.
pub fn verify_pak_integrity(data: &[u8]) -> bool {
    if data.len() < 32 {
        return false;
    }
    let (content, stored_hash) = data.split_at(data.len() - 32);
    let computed = sha256_hash(content);
    computed[..] == stored_hash[..]
}

/// Reader for a pak file. Loads the TOC into memory; blob data is read
/// on demand from the file.
pub struct PakReader {
    entries: Vec<PakEntry>,
    data: Vec<u8>,
}

impl PakReader {
    /// Open and read a pak file from disk.
    pub fn open(path: &Path) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }

    /// Parse a pak from an in-memory buffer. Supports both V1 and V2.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let mut cursor = &data[..];

        // -- Header --
        let mut magic = [0u8; 8];
        cursor.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a valid AMIGOPAK file",
            ));
        }

        let mut buf4 = [0u8; 4];
        cursor.read_exact(&mut buf4)?;
        let version = u32::from_le_bytes(buf4);
        if version != VERSION && version != VERSION_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported pak version: {version}"),
            ));
        }
        let is_v2 = version >= VERSION;

        cursor.read_exact(&mut buf4)?;
        let entry_count = u32::from_le_bytes(buf4) as usize;

        // -- TOC --
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let mut buf2 = [0u8; 2];
            cursor.read_exact(&mut buf2)?;
            let name_len = u16::from_le_bytes(buf2) as usize;

            let mut name_buf = vec![0u8; name_len];
            cursor.read_exact(&mut name_buf)?;
            let name = String::from_utf8(name_buf)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            let mut kind_buf = [0u8; 1];
            cursor.read_exact(&mut kind_buf)?;
            let kind = if is_v2 {
                AssetKind::from_u8(kind_buf[0])
            } else {
                AssetKind::from_v1(kind_buf[0])
            }
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unknown asset kind: {}", kind_buf[0]),
                )
            })?;

            // V2 has a flags byte after kind
            let flags = if is_v2 {
                let mut flags_buf = [0u8; 1];
                cursor.read_exact(&mut flags_buf)?;
                AssetFlags(flags_buf[0])
            } else {
                AssetFlags::NONE
            };

            let mut buf8 = [0u8; 8];
            cursor.read_exact(&mut buf8)?;
            let offset = u64::from_le_bytes(buf8);

            cursor.read_exact(&mut buf8)?;
            let size = u64::from_le_bytes(buf8);

            entries.push(PakEntry {
                name,
                kind,
                flags,
                offset,
                size,
            });
        }

        Ok(Self { entries, data })
    }

    /// List all entries in the pak.
    pub fn entries(&self) -> &[PakEntry] {
        &self.entries
    }

    /// Get raw bytes for an entry by name.
    pub fn read_entry(&self, name: &str) -> Option<&[u8]> {
        self.entries.iter().find(|e| e.name == name).map(|e| {
            let start = e.offset as usize;
            let end = start + e.size as usize;
            &self.data[start..end]
        })
    }

    /// Get raw bytes for an entry by index.
    pub fn read_entry_at(&self, index: usize) -> Option<&[u8]> {
        self.entries.get(index).map(|e| {
            let start = e.offset as usize;
            let end = start + e.size as usize;
            &self.data[start..end]
        })
    }

    /// Get all entries of a given kind.
    pub fn entries_of_kind(&self, kind: AssetKind) -> Vec<&PakEntry> {
        self.entries.iter().filter(|e| e.kind == kind).collect()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Total size in bytes of the pak file.
    pub fn total_size(&self) -> usize {
        self.data.len()
    }

    /// Verify the integrity hash (V2 paks only).
    pub fn verify_integrity(&self) -> bool {
        verify_pak_integrity(&self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pak read/write roundtrip ─────────────────────────────────

    #[test]
    fn roundtrip_pak_v2() {
        let mut writer = PakWriter::new();
        writer.add("player", AssetKind::Sprite, vec![0xFF; 64]);
        writer.add("jump.wav", AssetKind::Audio, vec![0xAA; 128]);
        writer.add("config.ron", AssetKind::Data, b"(speed: 5.0)".to_vec());

        let tmp = std::env::temp_dir().join("test_amigo_v2.pak");
        let size = writer.write_to(&tmp).unwrap();
        assert!(size > 0);

        let reader = PakReader::open(&tmp).unwrap();
        assert_eq!(reader.len(), 3);

        let sprite_data = reader.read_entry("player").unwrap();
        assert_eq!(sprite_data.len(), 64);
        assert!(sprite_data.iter().all(|&b| b == 0xFF));

        let audio_data = reader.read_entry("jump.wav").unwrap();
        assert_eq!(audio_data.len(), 128);

        let config_data = reader.read_entry("config.ron").unwrap();
        assert_eq!(config_data, b"(speed: 5.0)");

        assert!(reader.read_entry("nonexistent").is_none());

        let audio_entries = reader.entries_of_kind(AssetKind::Audio);
        assert_eq!(audio_entries.len(), 1);
        assert_eq!(audio_entries[0].name, "jump.wav");

        // Type tags should be 0x01+ now
        assert_eq!(reader.entries()[0].kind, AssetKind::Sprite);
        assert_eq!(reader.entries()[0].kind as u8, 0x01);
        assert_eq!(reader.entries()[1].kind as u8, 0x02);

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn empty_pak() {
        let writer = PakWriter::new();
        let tmp = std::env::temp_dir().join("test_amigo_empty_v2.pak");
        writer.write_to(&tmp).unwrap();

        let reader = PakReader::open(&tmp).unwrap();
        assert_eq!(reader.len(), 0);

        std::fs::remove_file(&tmp).ok();
    }

    // ── SHA-256 integrity ───────────────────────────────────────

    #[test]
    fn sha256_integrity() {
        let mut writer = PakWriter::new();
        writer.add("test", AssetKind::Data, b"hello world".to_vec());

        let tmp = std::env::temp_dir().join("test_amigo_sha.pak");
        writer.write_to(&tmp).unwrap();

        let reader = PakReader::open(&tmp).unwrap();
        assert!(reader.verify_integrity());

        // Tamper with the file
        let mut raw = std::fs::read(&tmp).unwrap();
        if let Some(b) = raw.get_mut(20) {
            *b ^= 0xFF;
        }
        std::fs::write(&tmp, &raw).unwrap();
        assert!(!verify_pak_integrity(&raw));

        std::fs::remove_file(&tmp).ok();
    }

    // ── Asset flags ─────────────────────────────────────────────

    #[test]
    fn asset_flags_bitmask() {
        let flags = AssetFlags(AssetFlags::COMPRESSED.0 | AssetFlags::ENCRYPTED.0);
        assert!(flags.is_compressed());
        assert!(flags.is_encrypted());
        assert!(!AssetFlags::NONE.is_compressed());
    }

    #[test]
    fn pak_with_flags() {
        let mut writer = PakWriter::new();
        writer.add_with_flags(
            "big_data",
            AssetKind::Data,
            AssetFlags::COMPRESSED,
            vec![0x42; 256],
        );

        let tmp = std::env::temp_dir().join("test_amigo_flags.pak");
        writer.write_to(&tmp).unwrap();

        let reader = PakReader::open(&tmp).unwrap();
        assert_eq!(reader.entries()[0].flags, AssetFlags::COMPRESSED);
        assert!(reader.entries()[0].flags.is_compressed());

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn sha256_known_vector() {
        // SHA-256 of empty string is well-known
        let hash = sha256_hash(b"");
        let expected = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(hash, expected);
    }
}
