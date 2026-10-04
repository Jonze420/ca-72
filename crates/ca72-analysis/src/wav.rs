//! Reading WAV files for analysis: any chunk layout, PCM (8, 16, 24, 32 bits) and IEEE
//! float (32, 64 bits), plain or extensible format.

/// Sample rate and channels.
pub fn read(bytes: &[u8]) -> Result<(u32, Vec<Vec<f32>>), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let u16_at = |i: usize| -> Option<u16> {
        Some(u16::from_le_bytes(bytes.get(i..i + 2)?.try_into().ok()?))
    };
    let u32_at = |i: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?))
    };
    let mut fmt: Option<(u16, usize, u32, usize)> = None;
    let mut data: Option<&[u8]> = None;
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32_at(pos + 4).ok_or("truncated chunk")? as usize;
        let body = pos + 8;
        let end = (body + size).min(bytes.len());
        match id {
            b"fmt " => {
                let mut tag = u16_at(body).ok_or("short fmt chunk")?;
                let channels = usize::from(u16_at(body + 2).ok_or("short fmt chunk")?);
                let rate = u32_at(body + 4).ok_or("short fmt chunk")?;
                let bits = usize::from(u16_at(body + 14).ok_or("short fmt chunk")?);
                if tag == 0xFFFE {
                    // WAVE_FORMAT_EXTENSIBLE: the sub-format GUID starts with the tag.
                    tag = u16_at(body + 24).ok_or("short extensible fmt chunk")?;
                }
                fmt = Some((tag, channels, rate, bits));
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        pos = body + size + (size & 1);
    }
    let (tag, ch, rate, bits) = fmt.ok_or("no fmt chunk")?;
    let data = data.ok_or("no data chunk")?;
    if ch == 0 {
        return Err("no channels".into());
    }
    let width = bits / 8;
    let sample = |b: &[u8]| -> Option<f32> {
        Some(match (tag, bits) {
            (1, 8) => (f32::from(b[0]) - 128.0) / 128.0,
            (1, 16) => f32::from(i16::from_le_bytes(b.try_into().ok()?)) / 32768.0,
            (1, 24) => {
                let v = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
                v as f32 / 8_388_608.0
            }
            (1, 32) => (f64::from(i32::from_le_bytes(b.try_into().ok()?)) / 2_147_483_648.0) as f32,
            (3, 32) => f32::from_le_bytes(b.try_into().ok()?),
            (3, 64) => f64::from_le_bytes(b.try_into().ok()?) as f32,
            _ => return None,
        })
    };
    if width == 0 || sample(&vec![0u8; width]).is_none() {
        return Err(format!(
            "unsupported WAV sample format {tag} with {bits} bits"
        ));
    }
    let frames = data.len() / (width * ch);
    let mut out = vec![Vec::with_capacity(frames); ch];
    for f in 0..frames {
        for (c, v) in out.iter_mut().enumerate() {
            let i = (f * ch + c) * width;
            v.push(sample(&data[i..i + width]).unwrap_or(0.0));
        }
    }
    Ok((rate, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(tag: u16, bits: u16, ch: u16, extra: &[u8], data: &[u8]) -> Vec<u8> {
        let mut b = b"RIFF\0\0\0\0WAVE".to_vec();
        b.extend_from_slice(b"LIST\x04\0\0\0abcd");
        b.extend_from_slice(b"fmt \x10\0\0\0");
        b.extend_from_slice(&tag.to_le_bytes());
        b.extend_from_slice(&ch.to_le_bytes());
        b.extend_from_slice(&44_100u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&bits.to_le_bytes());
        b.extend_from_slice(extra);
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(data);
        b
    }

    #[test]
    fn pcm_and_float_formats_read() {
        let pcm16 = wav(1, 16, 2, &[], &[0x00, 0x40, 0x00, 0xC0]);
        let (sr, ch) = read(&pcm16).unwrap();
        assert_eq!((sr, ch.len()), (44_100, 2));
        assert_eq!((ch[0][0], ch[1][0]), (0.5, -0.5));
        let pcm24 = wav(1, 24, 1, &[], &[0x00, 0x00, 0x40]);
        assert_eq!(read(&pcm24).unwrap().1[0][0], 0.5);
        let f32s = wav(3, 32, 1, &[], &0.25f32.to_le_bytes());
        assert_eq!(read(&f32s).unwrap().1[0][0], 0.25);
        assert!(read(&wav(2, 4, 1, &[], &[0])).is_err());
        assert!(read(b"nope").is_err());
    }
}
