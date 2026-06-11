//! Commodore Amiga IFF/ILBM codec (EA-IFF-85 container, interleaved
//! bitplanes).
//!
//! Container facts are authored from
//! `syntheses/commodore-amiga/amiga-workbench-intuition-gui.md` § 10
//! ("iffparse.library", EA-IFF-85 + ILBM chunk structure) and
//! `reference/by-system/commodore-amiga/amiga-libraries-overview.md`
//! ("FORM ILBM — bitmap images. Key chunks: BMHD, CAMG, CMAP, BODY"):
//!
//! - An IFF file is a tree of chunks: 4-byte ASCII ID, big-endian 32-bit
//!   size (excluding the 8-byte header), then the payload **padded to even
//!   length with a zero byte** (synthesis § 10.1).
//! - The outermost chunk is a `FORM`; the first 4 payload bytes are the form
//!   type — `ILBM` here. The FORM size therefore includes the type ID plus
//!   all child chunk headers, payloads, and pads
//!   (`amiga-libraries-overview.md`, "A FORM's size field includes the
//!   nested 4-char type identifier and the 4+4-byte headers of all child
//!   chunks and the pad bytes").
//! - An ILBM FORM typically contains `BMHD` (bitmap header), `CMAP`
//!   (colormap, RGB triples), `BODY` (pixel data), and optionally `CAMG`
//!   (Amiga viewmode) among others (synthesis § 10.1). Unknown chunks are
//!   skipped on decode.
//!
//! The field-by-field `BitMapHeader` layout, the row format, and the
//! ByteRun1 compression scheme are from the EA-IFF-85 ILBM specification
//! (Jerry Morrison, Electronic Arts, 1985–86 — the document reprinted in the
//! Amiga ROM Kernel Reference Manual: Devices appendix). That spec is
//! community knowledge here pending a `reference/` provenance entry — a
//! tracked follow-up.
//!
//! - **BMHD** (20 bytes): `w:u16 h:u16 x:i16 y:i16 nPlanes:u8 masking:u8
//!   compression:u8 pad1:u8 transparentColor:u16 xAspect:u8 yAspect:u8
//!   pageWidth:i16 pageHeight:i16`, all big-endian. This encoder writes
//!   `x = y = 0`, `masking = 0`, `pad1 = 0`, `transparentColor = 0`,
//!   `xAspect:yAspect = 10:11` (the spec's lores PAL pixel aspect), and
//!   `pageWidth/pageHeight` equal to the image size.
//! - **BODY**: rows top to bottom; within each row, one scanline per plane
//!   (plane 0 first); each plane scanline is `ceil(width / 8)` bytes
//!   **padded to a word (2-byte) boundary**, MSB = leftmost pixel.
//!   `compression = 1` packs each plane scanline independently with
//!   ByteRun1 (PackBits): control byte `n` as i8 — `0..=127` ⇒ copy the
//!   next `n + 1` bytes literally; `-1..=-127` ⇒ repeat the next byte
//!   `1 - n` times; `-128` ⇒ no-op.
//! - **CAMG** (4 bytes): the Amiga viewmode longword. The bits this layer
//!   documents are [`CAMG_LACE`] (0x0004) and [`CAMG_HIRES`] (0x8000), the
//!   `graphics/view.h` mode bits; write 0 for a lores non-laced image.
//!   The value is carried, not interpreted.
//!
//! Decode is tolerant where the wild demands it: unknown chunks are
//! skipped; `masking = 1` mask scanlines are skipped; a missing CMAP yields
//! an empty palette; bytes after the FORM's declared end are ignored.
//! Decode is strict where corruption hides: bad magic, truncated chunks,
//! `nPlanes` outside 1..=8, unknown compression values, dimensions above
//! [`MAX_DIMENSION`], and ByteRun1 runs that overrun their scanline are all
//! typed errors.

use super::{DecodeError, EncodeError};

/// Sanity cap on declared width and height, in pixels. Generous for any
/// period Amiga mode (max overscan SuperHires-laced is 1448×580) while
/// keeping `width × height` allocations bounded.
pub const MAX_DIMENSION: u16 = 4096;

/// CAMG bit: interlace (`graphics/view.h` `LACE`).
pub const CAMG_LACE: u32 = 0x0004;
/// CAMG bit: hires (`graphics/view.h` `HIRES`).
pub const CAMG_HIRES: u32 = 0x8000;

/// BMHD payload length in bytes.
const BMHD_LEN: usize = 20;

/// BODY compression scheme, per the BMHD `compression` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// `compression = 0`: raw scanlines.
    None,
    /// `compression = 1`: ByteRun1 (PackBits), per plane scanline.
    ByteRun1,
}

/// A decoded (or to-be-encoded) ILBM image in chunky indexed form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ilbm {
    /// Width in pixels (1..=[`MAX_DIMENSION`]).
    pub width: u16,
    /// Height in pixels (1..=[`MAX_DIMENSION`]).
    pub height: u16,
    /// Bitplane count, 1..=8. Pixel indices must fit in this many bits.
    pub n_planes: u8,
    /// CMAP palette as RGB triples; may hold fewer (or more) entries than
    /// `2^n_planes`. Empty means the file carried no CMAP (encode then
    /// omits the chunk).
    pub palette: Vec<[u8; 3]>,
    /// Chunky indexed pixels, row-major, `width × height` entries.
    pub pixels: Vec<u8>,
    /// The CAMG viewmode longword; 0 for lores non-laced (and for files
    /// without a CAMG chunk). Carried verbatim, not interpreted.
    pub camg: u32,
}

/// Bytes per plane scanline: `ceil(width / 8)` rounded up to a word
/// boundary, per the ILBM spec's row format.
#[must_use]
pub fn row_bytes(width: u16) -> usize {
    (usize::from(width)).div_ceil(16) * 2
}

/// Encode an [`Ilbm`] into a FORM ILBM byte stream with chunks BMHD, CMAP
/// (omitted when the palette is empty), CAMG, and BODY, in that order.
///
/// # Errors
///
/// - [`EncodeError::ValueOutOfRange`] when `width`/`height` are 0 or above
///   [`MAX_DIMENSION`], `n_planes` is outside 1..=8, the palette has more
///   than 256 entries, or any pixel index needs more than `n_planes` bits.
/// - [`EncodeError::WrongLength`] when `pixels` is not `width × height`
///   entries.
pub fn encode(image: &Ilbm, compression: Compression) -> Result<Vec<u8>, EncodeError> {
    validate_for_encode(image)?;

    let width = usize::from(image.width);
    let height = usize::from(image.height);
    let planes = usize::from(image.n_planes);
    let row_len = row_bytes(image.width);

    // BODY: per row, per plane, one (possibly packed) scanline.
    let mut body = Vec::new();
    let mut plane_row = vec![0u8; row_len];
    for y in 0..height {
        let row_pixels = &image.pixels[y * width..(y + 1) * width];
        for plane in 0..planes {
            plane_row.fill(0);
            for (x, &pixel) in row_pixels.iter().enumerate() {
                if pixel & (1 << plane) != 0 {
                    plane_row[x / 8] |= 0x80 >> (x % 8);
                }
            }
            match compression {
                Compression::None => body.extend_from_slice(&plane_row),
                Compression::ByteRun1 => pack_byte_run1(&plane_row, &mut body),
            }
        }
    }

    let mut form = Vec::new();
    form.extend_from_slice(b"ILBM");
    push_chunk(&mut form, b"BMHD", &bmhd_payload(image, compression));
    if !image.palette.is_empty() {
        let mut cmap = Vec::with_capacity(image.palette.len() * 3);
        for rgb in &image.palette {
            cmap.extend_from_slice(rgb);
        }
        push_chunk(&mut form, b"CMAP", &cmap);
    }
    push_chunk(&mut form, b"CAMG", &image.camg.to_be_bytes());
    push_chunk(&mut form, b"BODY", &body);

    let mut out = Vec::with_capacity(8 + form.len());
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(u32::try_from(form.len()).unwrap_or(u32::MAX)).to_be_bytes());
    out.extend_from_slice(&form);
    Ok(out)
}

/// Decode a FORM ILBM byte stream into chunky indexed form.
///
/// # Errors
///
/// - [`DecodeError::BadMagic`] — not a `FORM`, or the form type is not
///   `ILBM`.
/// - [`DecodeError::Truncated`] — input ends inside the FORM header, a
///   chunk header, a chunk payload, or the BODY data.
/// - [`DecodeError::Unsupported`] — `nPlanes` outside 1..=8, compression
///   other than 0/1, masking other than 0..=3, zero width/height, or a
///   CMAP longer than 256 entries.
/// - [`DecodeError::DimensionsTooLarge`] — width or height above
///   [`MAX_DIMENSION`].
/// - [`DecodeError::MissingChunk`] — no BMHD before BODY, or no BODY at
///   all.
/// - [`DecodeError::Corrupt`] — CMAP length not a multiple of 3, or a
///   ByteRun1 run overrunning its scanline.
pub fn decode(bytes: &[u8]) -> Result<Ilbm, DecodeError> {
    if bytes.len() < 12 {
        return Err(DecodeError::Truncated {
            what: "FORM header",
        });
    }
    if &bytes[0..4] != b"FORM" {
        return Err(DecodeError::BadMagic { what: "FORM" });
    }
    let form_size = be_u32(&bytes[4..8]) as usize;
    if form_size < 4 || bytes.len() < 8 + form_size {
        return Err(DecodeError::Truncated {
            what: "FORM payload",
        });
    }
    if &bytes[8..12] != b"ILBM" {
        return Err(DecodeError::BadMagic { what: "ILBM" });
    }

    let form_end = 8 + form_size;
    let mut pos = 12;
    let mut header: Option<Bmhd> = None;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut camg = 0u32;
    // The decoded BODY with the BMHD geometry it was decoded against
    // (`header` may in principle be replaced by a later BMHD chunk):
    // (width, height, n_planes, pixels).
    let mut body: Option<(u16, u16, u8, Vec<u8>)> = None;

    while pos + 8 <= form_end {
        let id = &bytes[pos..pos + 4];
        let size = be_u32(&bytes[pos + 4..pos + 8]) as usize;
        let data_start = pos + 8;
        let data_end = data_start.checked_add(size).ok_or(DecodeError::Corrupt {
            what: "chunk size overflows",
        })?;
        if data_end > form_end {
            return Err(DecodeError::Truncated {
                what: "chunk payload",
            });
        }
        let data = &bytes[data_start..data_end];

        match id {
            b"BMHD" => header = Some(parse_bmhd(data)?),
            b"CMAP" => {
                if !size.is_multiple_of(3) {
                    return Err(DecodeError::Corrupt {
                        what: "CMAP length not a multiple of 3",
                    });
                }
                if size / 3 > 256 {
                    return Err(DecodeError::Unsupported {
                        what: "CMAP entry count",
                        value: (size / 3) as u32,
                    });
                }
                palette = data.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            b"CAMG" => {
                if size < 4 {
                    return Err(DecodeError::Truncated {
                        what: "CAMG payload",
                    });
                }
                camg = be_u32(&data[0..4]);
            }
            b"BODY" => {
                let bmhd = header
                    .as_ref()
                    .ok_or(DecodeError::MissingChunk { id: "BMHD" })?;
                body = Some((
                    bmhd.width,
                    bmhd.height,
                    bmhd.n_planes,
                    decode_body(bmhd, data)?,
                ));
            }
            _ => {} // Unknown chunk: skip.
        }

        pos = data_end + (size & 1); // Even-length chunk padding.
    }

    let (width, height, n_planes, pixels) = body.ok_or(DecodeError::MissingChunk { id: "BODY" })?;
    Ok(Ilbm {
        width,
        height,
        n_planes,
        palette,
        pixels,
        camg,
    })
}

/// Parsed BMHD fields the decoder needs.
struct Bmhd {
    width: u16,
    height: u16,
    n_planes: u8,
    masking: u8,
    compression: u8,
}

fn parse_bmhd(data: &[u8]) -> Result<Bmhd, DecodeError> {
    if data.len() < BMHD_LEN {
        return Err(DecodeError::Truncated {
            what: "BMHD payload",
        });
    }
    let width = u16::from_be_bytes([data[0], data[1]]);
    let height = u16::from_be_bytes([data[2], data[3]]);
    let n_planes = data[8];
    let masking = data[9];
    let compression = data[10];

    if width == 0 {
        return Err(DecodeError::Unsupported {
            what: "BMHD width",
            value: 0,
        });
    }
    if height == 0 {
        return Err(DecodeError::Unsupported {
            what: "BMHD height",
            value: 0,
        });
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(DecodeError::DimensionsTooLarge { width, height });
    }
    if n_planes == 0 || n_planes > 8 {
        return Err(DecodeError::Unsupported {
            what: "BMHD nPlanes",
            value: u32::from(n_planes),
        });
    }
    if masking > 3 {
        return Err(DecodeError::Unsupported {
            what: "BMHD masking",
            value: u32::from(masking),
        });
    }
    if compression > 1 {
        return Err(DecodeError::Unsupported {
            what: "BMHD compression",
            value: u32::from(compression),
        });
    }
    Ok(Bmhd {
        width,
        height,
        n_planes,
        masking,
        compression,
    })
}

/// Decode a BODY chunk's scanlines into chunky indexed pixels, row-major
/// `width × height` entries.
fn decode_body(bmhd: &Bmhd, body: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let width = usize::from(bmhd.width);
    let height = usize::from(bmhd.height);
    let planes = usize::from(bmhd.n_planes);
    let row_len = row_bytes(bmhd.width);
    // masking = 1 (hasMask) interleaves one extra mask scanline per row,
    // which this chunky decoder skips. masking = 2/3 (transparent colour /
    // lasso) carry no extra plane data.
    let scanlines_per_row = planes + usize::from(bmhd.masking == 1);

    let mut pixels = vec![0u8; width * height];
    let mut cursor = 0usize;
    let mut plane_row = vec![0u8; row_len];

    for y in 0..height {
        for scanline in 0..scanlines_per_row {
            match bmhd.compression {
                0 => {
                    let end = cursor
                        .checked_add(row_len)
                        .filter(|&e| e <= body.len())
                        .ok_or(DecodeError::Truncated {
                            what: "BODY scanline",
                        })?;
                    plane_row.copy_from_slice(&body[cursor..end]);
                    cursor = end;
                }
                _ => unpack_byte_run1(body, &mut cursor, &mut plane_row)?,
            }
            if scanline >= planes {
                continue; // Mask scanline: skipped.
            }
            let row_pixels = &mut pixels[y * width..(y + 1) * width];
            for (x, pixel) in row_pixels.iter_mut().enumerate() {
                if plane_row[x / 8] & (0x80 >> (x % 8)) != 0 {
                    *pixel |= 1 << scanline;
                }
            }
        }
    }
    // Trailing BODY bytes (some writers over-pad) are tolerated.

    Ok(pixels)
}

/// Unpack exactly one ByteRun1 scanline (`out.len()` bytes) from `body`,
/// advancing `cursor`. Runs and literals must not cross the scanline
/// boundary — every standard writer (DPaint, iffparse-based tools, netpbm)
/// packs per scanline.
fn unpack_byte_run1(body: &[u8], cursor: &mut usize, out: &mut [u8]) -> Result<(), DecodeError> {
    let mut filled = 0usize;
    while filled < out.len() {
        let control = *body.get(*cursor).ok_or(DecodeError::Truncated {
            what: "ByteRun1 control byte",
        })? as i8;
        *cursor += 1;
        if control == -128 {
            continue; // No-op, per the spec.
        }
        if control >= 0 {
            let count = usize::from(control.unsigned_abs()) + 1;
            if filled + count > out.len() {
                return Err(DecodeError::Corrupt {
                    what: "ByteRun1 literal overruns scanline",
                });
            }
            let end = cursor
                .checked_add(count)
                .filter(|&e| e <= body.len())
                .ok_or(DecodeError::Truncated {
                    what: "ByteRun1 literal bytes",
                })?;
            out[filled..filled + count].copy_from_slice(&body[*cursor..end]);
            *cursor = end;
            filled += count;
        } else {
            let count = usize::from(control.unsigned_abs()) + 1;
            if filled + count > out.len() {
                return Err(DecodeError::Corrupt {
                    what: "ByteRun1 run overruns scanline",
                });
            }
            let value = *body.get(*cursor).ok_or(DecodeError::Truncated {
                what: "ByteRun1 run byte",
            })?;
            *cursor += 1;
            out[filled..filled + count].fill(value);
            filled += count;
        }
    }
    Ok(())
}

/// Pack one scanline with ByteRun1. Deterministic policy: encode a run when
/// 3 or more equal bytes follow (the spec's break-even guidance: a 2-run
/// inside a literal costs the same as a run but avoids breaking the
/// literal); literals and runs cap at 128 bytes; `-128` is never emitted.
fn pack_byte_run1(row: &[u8], out: &mut Vec<u8>) {
    let mut i = 0usize;
    while i < row.len() {
        let run = run_length(row, i);
        if run >= 3 {
            // 257 - run encodes -(run - 1) for run in 3..=128.
            out.push((257 - run) as u8);
            out.push(row[i]);
            i += run;
            continue;
        }
        // Literal block: absorb short runs until a 3-run starts or 128 bytes.
        let start = i;
        i += run;
        while i < row.len() && i - start < 128 {
            let next = run_length(row, i);
            if next >= 3 {
                break;
            }
            i += next;
        }
        i = i.min(start + 128);
        out.push((i - start - 1) as u8);
        out.extend_from_slice(&row[start..i]);
    }
}

/// Length of the equal-byte run starting at `i`, capped at 128.
fn run_length(row: &[u8], i: usize) -> usize {
    let mut len = 1;
    while i + len < row.len() && row[i + len] == row[i] && len < 128 {
        len += 1;
    }
    len
}

fn bmhd_payload(image: &Ilbm, compression: Compression) -> [u8; BMHD_LEN] {
    let mut p = [0u8; BMHD_LEN];
    p[0..2].copy_from_slice(&image.width.to_be_bytes());
    p[2..4].copy_from_slice(&image.height.to_be_bytes());
    // x, y origin: 0 (bytes 4..8 already zero).
    p[8] = image.n_planes;
    p[9] = 0; // masking = mskNone
    p[10] = match compression {
        Compression::None => 0,
        Compression::ByteRun1 => 1,
    };
    // pad1 = 0, transparentColor = 0 (bytes 11..14 already zero).
    p[14] = 10; // xAspect: lores PAL 10:11
    p[15] = 11; // yAspect
    p[16..18].copy_from_slice(&image.width.to_be_bytes()); // pageWidth
    p[18..20].copy_from_slice(&image.height.to_be_bytes()); // pageHeight
    p
}

/// Append a chunk (ID, big-endian size, payload, even pad) to `out`.
fn push_chunk(out: &mut Vec<u8>, id: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(u32::try_from(payload.len()).unwrap_or(u32::MAX)).to_be_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0); // Even-length chunk padding (synthesis § 10.1).
    }
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn validate_for_encode(image: &Ilbm) -> Result<(), EncodeError> {
    if image.width == 0 || image.width > MAX_DIMENSION {
        return Err(EncodeError::ValueOutOfRange {
            what: "ILBM width",
            value: u32::from(image.width),
            min: 1,
            max: u32::from(MAX_DIMENSION),
        });
    }
    if image.height == 0 || image.height > MAX_DIMENSION {
        return Err(EncodeError::ValueOutOfRange {
            what: "ILBM height",
            value: u32::from(image.height),
            min: 1,
            max: u32::from(MAX_DIMENSION),
        });
    }
    if image.n_planes == 0 || image.n_planes > 8 {
        return Err(EncodeError::ValueOutOfRange {
            what: "ILBM nPlanes",
            value: u32::from(image.n_planes),
            min: 1,
            max: 8,
        });
    }
    if image.palette.len() > 256 {
        return Err(EncodeError::ValueOutOfRange {
            what: "ILBM palette entries",
            value: u32::try_from(image.palette.len()).unwrap_or(u32::MAX),
            min: 0,
            max: 256,
        });
    }
    let expected = usize::from(image.width) * usize::from(image.height);
    if image.pixels.len() != expected {
        return Err(EncodeError::WrongLength {
            what: "ILBM pixels",
            expected,
            actual: image.pixels.len(),
        });
    }
    let max_index = if image.n_planes == 8 {
        u8::MAX
    } else {
        (1u8 << image.n_planes) - 1
    };
    if let Some(&bad) = image.pixels.iter().find(|&&p| p > max_index) {
        return Err(EncodeError::ValueOutOfRange {
            what: "ILBM pixel index",
            value: u32::from(bad),
            min: 0,
            max: u32::from(max_index),
        });
    }
    Ok(())
}
