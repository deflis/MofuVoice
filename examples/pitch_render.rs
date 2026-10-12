//! Offline pitch rendering. Accepts mono 48 kHz PCM16 WAV only.
use rust_low_latency_voice_changer::{
    pitch::{PitchShifter, LATENCY},
    RATE,
};
use std::{error::Error, fs, path::Path};

fn read_wav(path: &Path) -> Result<Vec<f32>, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Expected a RIFF WAV file".into());
    }
    let end = 8_usize
        .checked_add(u32::from_le_bytes(bytes[4..8].try_into()?) as usize)
        .ok_or("RIFF size overflow")?;
    if end != bytes.len() {
        return Err("RIFF size does not match file length".into());
    }
    let mut position = 12;
    let mut format = false;
    let mut samples = None;
    while position + 8 <= end {
        let size = u32::from_le_bytes(bytes[position + 4..position + 8].try_into()?) as usize;
        let start = position + 8;
        let stop = start.checked_add(size).ok_or("WAV chunk size overflow")?;
        let chunk = bytes.get(start..stop).ok_or("Truncated WAV chunk")?;
        match &bytes[position..position + 4] {
            b"fmt " => {
                if format || size < 16 {
                    return Err("Invalid or repeated fmt chunk".into());
                }
                let encoding = u16::from_le_bytes(chunk[0..2].try_into()?);
                let channels = u16::from_le_bytes(chunk[2..4].try_into()?);
                let rate = u32::from_le_bytes(chunk[4..8].try_into()?);
                let byte_rate = u32::from_le_bytes(chunk[8..12].try_into()?);
                let alignment = u16::from_le_bytes(chunk[12..14].try_into()?);
                let bits = u16::from_le_bytes(chunk[14..16].try_into()?);
                if (encoding, channels, rate, byte_rate, alignment, bits)
                    != (1, 1, RATE, RATE * 2, 2, 16)
                {
                    return Err(
                        "Use mono 48000 Hz PCM16 WAV; no implicit conversion is performed".into(),
                    );
                }
                format = true;
            }
            b"data" => {
                if samples.is_some() || !size.is_multiple_of(2) {
                    return Err("Invalid or repeated data chunk".into());
                }
                samples = Some(
                    chunk
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.0)
                        .collect(),
                );
            }
            _ => {}
        }
        position = stop + size % 2;
    }
    if position != end || !format {
        return Err("Malformed WAV or missing format chunk".into());
    }
    samples.ok_or_else(|| "Missing WAV data chunk".into())
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<(), Box<dyn Error>> {
    let size = u32::try_from(samples.len().checked_mul(2).ok_or("WAV too large")?)?;
    let riff_size = size.checked_add(36).ok_or("WAV too large")?;
    let mut bytes = Vec::with_capacity(44 + size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());
    for &sample in samples {
        bytes.extend_from_slice(&((sample as f64 * 32768.0).round() as i16).to_le_bytes());
    }
    // Avoid overwriting listening material from an earlier comparison.
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&bytes)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "Usage: pitch_render INPUT.wav OUTPUT_DIRECTORY PITCH_SEMITONES FORMANT_SEMITONES"
                .into(),
        );
    }
    let mut input = read_wav(Path::new(&args[0]))?;
    let count = input.len();
    if count == 0 {
        return Err("Input WAV is empty".into());
    }
    let pitch: f64 = args[2].parse()?;
    let shift: f64 = args[3].parse()?;
    let mut dsp = PitchShifter::new(pitch, true, shift)?;
    let directory = Path::new(&args[1]);
    for name in ["input.wav", "processed.wav"] {
        if directory.join(name).exists() {
            return Err(
                format!("Output already exists: {}", directory.join(name).display()).into(),
            );
        }
    }
    input.resize(count + LATENCY * 2, 0.0);
    let mut output = vec![0.0; input.len()];
    for (a, b) in input.chunks(512).zip(output.chunks_mut(512)) {
        dsp.process_block(a, b);
    }
    let delay = if pitch == 0.0 && shift == 0.0 {
        0
    } else {
        LATENCY
    };
    let output = &output[delay..delay + count];
    let clipped = output.iter().filter(|&&x| x.abs() >= 1.0).count();
    fs::create_dir_all(directory)?;
    write_wav(&directory.join("input.wav"), &input[..count])?;
    write_wav(&directory.join("processed.wav"), output)?;
    println!("Wrote input.wav and processed.wav. Output trimmed by {delay} samples. Full-scale samples: {clipped}/{count}. Levels are not loudness-matched. Compare vowels, consonants, and voice transitions.");
    Ok(())
}
