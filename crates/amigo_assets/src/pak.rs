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
const VERSION: u32 = 1;

/// The kind of asset stored in the pak file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AssetKind {
    Sprite = 0,
    Audio = 1,
    Data = 2,
    Level = 3,
    Font = 4,
    AtlasImage = 5,
    AtlasManifest = 6,
}

impl AssetKind {
    fn from_u8(v: u8) -> Option<Self> {
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

/// A single entry in the pak table of contents.
#[derive(Clone, Debug)]
pub struct PakEntry {
    pub name: String,
    pub kind: AssetKind,
    pub offset: u64,
    pub size: u64,
}

/// Builder for creating a pak file.
pub struct PakWriter {
    entries: Vec<(String, AssetKind, Vec<u8>)>,
}

impl PakWriter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an asset from raw bytes.
    pub fn add(&mut self, name: impl Into<String>, kind: AssetKind, data: Vec<u8>) {
        self.entries.push((name.into(), kind, data));
    }

    /// Add an asset from a file on disk.
    pub fn add_file(&mut self, name: impl Into<String>, kind: AssetKind, path: &Path) -> io::Result<()> {
        let data = std::fs::read(path)?;
        self.add(name, kind, data);
        Ok(())
    }

    /// Number of entries added so far.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Write the pak file to disk.
    pub fn write_to(&self, path: &Path) -> io::Result<u64> {
        let mut file = std::fs::File::create(path)?;

        // -- Header --
        file.write_all(MAGIC)?;
        file.write_all(&VERSION.to_le_bytes())?;
        file.write_all(&(self.entries.len() as u32).to_le_bytes())?;

        // -- Compute TOC size so we know blob offsets --
        let header_size: u64 = 8 + 4 + 4; // magic + version + count
        let mut toc_size: u64 = 0;
        for (name, _, _) in &self.entries {
            // 2 bytes name_len + name bytes + 1 byte kind + 8 bytes offset + 8 bytes size
            toc_size += 2 + name.len() as u64 + 1 + 8 + 8;
        }

        let blob_start = header_size + toc_size;
        let mut current_offset = blob_start;

        // -- Write TOC --
        for (name, kind, data) in &self.entries {
            let name_bytes = name.as_bytes();
            file.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
            file.write_all(name_bytes)?;
            file.write_all(&[*kind as u8])?;
            file.write_all(&current_offset.to_le_bytes())?;
            file.write_all(&(data.len() as u64).to_le_bytes())?;
            current_offset += data.len() as u64;
        }

        // -- Write blob data --
        for (_, _, data) in &self.entries {
            file.write_all(data)?;
        }

        let total_size = current_offset;
        Ok(total_size)
    }
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

    /// Parse a pak from an in-memory buffer.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let mut cursor = &data[..];

        // -- Header --
        let mut magic = [0u8; 8];
        cursor.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a valid AMIGOPAK file"));
        }

        let mut buf4 = [0u8; 4];
        cursor.read_exact(&mut buf4)?;
        let version = u32::from_le_bytes(buf4);
        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported pak version: {version}"),
            ));
        }

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
            let kind = AssetKind::from_u8(kind_buf[0]).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Unknown asset kind: {}", kind_buf[0]))
            })?;

            let mut buf8 = [0u8; 8];
            cursor.read_exact(&mut buf8)?;
            let offset = u64::from_le_bytes(buf8);

            cursor.read_exact(&mut buf8)?;
            let size = u64::from_le_bytes(buf8);

            entries.push(PakEntry {
                name,
                kind,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_pak() {
        let mut writer = PakWriter::new();
        writer.add("player", AssetKind::Sprite, vec![0xFF; 64]);
        writer.add("jump.wav", AssetKind::Audio, vec![0xAA; 128]);
        writer.add("config.ron", AssetKind::Data, b"(speed: 5.0)".to_vec());

        let tmp = std::env::temp_dir().join("test_amigo.pak");
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

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn empty_pak() {
        let writer = PakWriter::new();
        let tmp = std::env::temp_dir().join("test_amigo_empty.pak");
        writer.write_to(&tmp).unwrap();

        let reader = PakReader::open(&tmp).unwrap();
        assert_eq!(reader.len(), 0);

        std::fs::remove_file(&tmp).ok();
    }
}
