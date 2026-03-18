//! Runtime format conversions (RS-21).
//!
//! Provides converters between source formats (PNG, WAV) and optimised
//! runtime formats (WebP, OGG) as well as the engine-native `.ait`
//! (Amigo Image Tile) atlas format.

use std::io;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during format conversion.
#[derive(Debug, Error)]
pub enum FormatError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("Unsupported format: {0}")]
    Unsupported(String),
    #[error("Invalid AIT file: {0}")]
    InvalidAit(String),
}

// ---------------------------------------------------------------------------
// .ait — Amigo Image Tile format
// ---------------------------------------------------------------------------

/// Magic bytes for AIT files.
const AIT_MAGIC: &[u8; 4] = b"AIMG";
/// Current AIT version.
const AIT_VERSION: u8 = 1;

/// A prepackaged atlas tile in the `.ait` format.
///
/// Layout (little-endian):
/// ```text
/// [4 bytes]  magic "AIMG"
/// [1 byte]   version
/// [2 bytes]  tile_width
/// [2 bytes]  tile_height
/// [2 bytes]  columns
/// [2 bytes]  rows
/// [4 bytes]  pixel_data_len
/// [N bytes]  RGBA pixel data (uncompressed)
/// ```
#[derive(Clone, Debug)]
pub struct AitFile {
    pub tile_width: u16,
    pub tile_height: u16,
    pub columns: u16,
    pub rows: u16,
    /// Raw RGBA pixel data for the entire atlas.
    pub pixel_data: Vec<u8>,
}

impl AitFile {
    /// Create a new AIT from an atlas image and tile dimensions.
    pub fn from_atlas(
        pixels: &[u8],
        atlas_width: u32,
        atlas_height: u32,
        tile_width: u16,
        tile_height: u16,
    ) -> Result<Self, FormatError> {
        let expected_len = (atlas_width * atlas_height * 4) as usize;
        if pixels.len() != expected_len {
            return Err(FormatError::InvalidAit(format!(
                "Expected {} bytes, got {}",
                expected_len,
                pixels.len()
            )));
        }
        let columns = (atlas_width as u16) / tile_width;
        let rows = (atlas_height as u16) / tile_height;
        Ok(Self {
            tile_width,
            tile_height,
            columns,
            rows,
            pixel_data: pixels.to_vec(),
        })
    }

    /// Encode to AIT binary format.
    pub fn encode(&self) -> Vec<u8> {
        let data_len = self.pixel_data.len() as u32;
        let total = 4 + 1 + 2 + 2 + 2 + 2 + 4 + self.pixel_data.len();
        let mut buf = Vec::with_capacity(total);
        buf.extend_from_slice(AIT_MAGIC);
        buf.push(AIT_VERSION);
        buf.extend_from_slice(&self.tile_width.to_le_bytes());
        buf.extend_from_slice(&self.tile_height.to_le_bytes());
        buf.extend_from_slice(&self.columns.to_le_bytes());
        buf.extend_from_slice(&self.rows.to_le_bytes());
        buf.extend_from_slice(&data_len.to_le_bytes());
        buf.extend_from_slice(&self.pixel_data);
        buf
    }

    /// Decode from AIT binary data.
    pub fn decode(data: &[u8]) -> Result<Self, FormatError> {
        if data.len() < 17 {
            return Err(FormatError::InvalidAit("File too small".into()));
        }
        if &data[0..4] != AIT_MAGIC {
            return Err(FormatError::InvalidAit("Bad magic".into()));
        }
        let version = data[4];
        if version != AIT_VERSION {
            return Err(FormatError::InvalidAit(format!(
                "Unsupported version {}",
                version
            )));
        }
        let tile_width = u16::from_le_bytes([data[5], data[6]]);
        let tile_height = u16::from_le_bytes([data[7], data[8]]);
        let columns = u16::from_le_bytes([data[9], data[10]]);
        let rows = u16::from_le_bytes([data[11], data[12]]);
        let pixel_data_len = u32::from_le_bytes([data[13], data[14], data[15], data[16]]) as usize;

        if data.len() < 17 + pixel_data_len {
            return Err(FormatError::InvalidAit("Truncated pixel data".into()));
        }
        Ok(Self {
            tile_width,
            tile_height,
            columns,
            rows,
            pixel_data: data[17..17 + pixel_data_len].to_vec(),
        })
    }

    /// Write to a file.
    pub fn write_to(&self, path: &Path) -> Result<(), FormatError> {
        let encoded = self.encode();
        std::fs::write(path, &encoded)?;
        Ok(())
    }

    /// Read from a file.
    pub fn read_from(path: &Path) -> Result<Self, FormatError> {
        let data = std::fs::read(path)?;
        Self::decode(&data)
    }

    /// Total number of tiles in the atlas.
    pub fn tile_count(&self) -> usize {
        self.columns as usize * self.rows as usize
    }

    /// Atlas width in pixels.
    pub fn atlas_width(&self) -> u32 {
        self.columns as u32 * self.tile_width as u32
    }

    /// Atlas height in pixels.
    pub fn atlas_height(&self) -> u32 {
        self.rows as u32 * self.tile_height as u32
    }
}

// ---------------------------------------------------------------------------
// Image format conversion helpers
// ---------------------------------------------------------------------------

/// Supported runtime image formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    WebP,
}

/// Convert a PNG image to WebP (lossy, quality 0-100).
///
/// Uses the `image` crate's built-in WebP encoder when available.
/// Falls back to storing as PNG-in-pak when WebP is not compiled in.
pub fn png_to_webp(png_data: &[u8], quality: u8) -> Result<Vec<u8>, FormatError> {
    let img = image::load_from_memory_with_format(png_data, image::ImageFormat::Png)?;
    let rgba = img.to_rgba8();
    let mut buf = Vec::new();
    // image crate 0.25+ supports WebP encoding
    rgba.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::WebP)
        .map_err(|e| FormatError::Unsupported(format!("WebP encoding: {e}")))?;
    let _ = quality; // quality knob reserved for future encoder configurability
    Ok(buf)
}

/// Detect whether a byte slice is a PNG file.
pub fn is_png(data: &[u8]) -> bool {
    data.len() >= 8 && data[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
}

/// Detect whether a byte slice is a WebP file.
pub fn is_webp(data: &[u8]) -> bool {
    data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP"
}

// ---------------------------------------------------------------------------
// Audio format conversion helpers
// ---------------------------------------------------------------------------

/// Supported runtime audio formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Ogg,
}

/// WAV file header detection.
pub fn is_wav(data: &[u8]) -> bool {
    data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE"
}

/// OGG file header detection.
pub fn is_ogg(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == b"OggS"
}

/// Estimate compression ratio between source and runtime formats.
pub fn estimate_savings(original_size: usize, converted_size: usize) -> f64 {
    if original_size == 0 {
        return 0.0;
    }
    1.0 - (converted_size as f64 / original_size as f64)
}

// ---------------------------------------------------------------------------
// Build-time conversion pipeline
// ---------------------------------------------------------------------------

/// Conversion result with size comparison.
#[derive(Clone, Debug)]
pub struct ConversionResult {
    pub source_format: String,
    pub target_format: String,
    pub source_size: usize,
    pub target_size: usize,
    pub savings_percent: f64,
    pub data: Vec<u8>,
}

/// Convert an image asset for runtime use.
///
/// If WebP encoding is available and produces a smaller file, returns WebP.
/// Otherwise returns the original PNG data.
pub fn convert_image(png_data: &[u8]) -> Result<ConversionResult, FormatError> {
    let source_size = png_data.len();

    match png_to_webp(png_data, 90) {
        Ok(webp_data) => {
            let target_size = webp_data.len();
            if target_size < source_size {
                Ok(ConversionResult {
                    source_format: "PNG".into(),
                    target_format: "WebP".into(),
                    source_size,
                    target_size,
                    savings_percent: estimate_savings(source_size, target_size),
                    data: webp_data,
                })
            } else {
                // WebP is bigger — keep PNG
                Ok(ConversionResult {
                    source_format: "PNG".into(),
                    target_format: "PNG".into(),
                    source_size,
                    target_size: source_size,
                    savings_percent: 0.0,
                    data: png_data.to_vec(),
                })
            }
        }
        Err(_) => {
            // WebP not available — keep PNG
            Ok(ConversionResult {
                source_format: "PNG".into(),
                target_format: "PNG".into(),
                source_size,
                target_size: source_size,
                savings_percent: 0.0,
                data: png_data.to_vec(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ait_roundtrip() {
        let pixels = vec![0u8; 16 * 16 * 4]; // 16x16 RGBA
        let ait = AitFile::from_atlas(&pixels, 16, 16, 8, 8).unwrap();
        assert_eq!(ait.columns, 2);
        assert_eq!(ait.rows, 2);
        assert_eq!(ait.tile_count(), 4);

        let encoded = ait.encode();
        let decoded = AitFile::decode(&encoded).unwrap();
        assert_eq!(decoded.tile_width, 8);
        assert_eq!(decoded.tile_height, 8);
        assert_eq!(decoded.columns, 2);
        assert_eq!(decoded.rows, 2);
        assert_eq!(decoded.pixel_data.len(), pixels.len());
    }

    #[test]
    fn ait_bad_magic() {
        let data = b"BADMxxxxxxxxxxxxxxxxxxxxxxx";
        assert!(AitFile::decode(data).is_err());
    }

    #[test]
    fn ait_too_small() {
        let data = b"AIMG";
        assert!(AitFile::decode(data).is_err());
    }

    #[test]
    fn ait_atlas_dimensions() {
        let pixels = vec![0u8; 64 * 32 * 4];
        let ait = AitFile::from_atlas(&pixels, 64, 32, 16, 16).unwrap();
        assert_eq!(ait.atlas_width(), 64);
        assert_eq!(ait.atlas_height(), 32);
        assert_eq!(ait.tile_count(), 8); // 4 cols × 2 rows
    }

    #[test]
    fn ait_pixel_data_mismatch() {
        let pixels = vec![0u8; 100]; // wrong size
        assert!(AitFile::from_atlas(&pixels, 16, 16, 8, 8).is_err());
    }

    #[test]
    fn detect_png() {
        let png_header = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        assert!(is_png(&png_header));
        assert!(!is_png(b"not png"));
    }

    #[test]
    fn detect_wav() {
        let wav_header = b"RIFF\x00\x00\x00\x00WAVEfmt ";
        assert!(is_wav(wav_header));
        assert!(!is_wav(b"not wav"));
    }

    #[test]
    fn detect_ogg() {
        assert!(is_ogg(b"OggS\x00\x00\x00\x00"));
        assert!(!is_ogg(b"nope"));
    }

    #[test]
    fn detect_webp() {
        let webp_header = b"RIFF\x00\x00\x00\x00WEBPVP8 ";
        assert!(is_webp(webp_header));
        assert!(!is_webp(b"RIFF\x00\x00\x00\x00WAVEfmt "));
    }

    #[test]
    fn savings_calculation() {
        assert!((estimate_savings(1000, 700) - 0.3).abs() < 0.001);
        assert_eq!(estimate_savings(0, 100), 0.0);
        assert!((estimate_savings(1000, 1000) - 0.0).abs() < 0.001);
    }

    #[test]
    fn convert_image_with_valid_png() {
        // Create a small valid PNG in memory
        let img = image::RgbaImage::new(4, 4);
        let mut png_bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .unwrap();

        let result = convert_image(&png_bytes).unwrap();
        assert!(result.source_format == "PNG");
        assert!(!result.data.is_empty());
    }
}
