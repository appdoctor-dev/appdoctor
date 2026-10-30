//! The video's sound: effects synthesized here (no samples shipped, nothing
//! downloaded), sound files the user or their agent placed on the timeline,
//! mixed to 48 kHz stereo. The export encodes it to AAC at 256 kbps, as
//! Apple asks of app previews, and muxes it in.

use std::f32::consts::{PI, TAU};
use std::path::Path;
use std::process::Command;

use appdoctor_screenshots::{Design, SoundEffect, motion};

use crate::{Error, Result};

pub const RATE: u32 = 48_000;

/// Whether a design's video gets an audio track: app previews always (a
/// silent one if nothing plays, matching Apple's audio spec), others when
/// something plays.
pub fn wants_audio(design: &Design) -> bool {
    let video = match &design.video {
        Some(video) => video,
        None => return false,
    };
    design.target.is_screenshot() || video.sound_effects || !video.audio.is_empty()
}

/// A tiny deterministic noise source, so effects sound the same each time.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2. - 1.
    }
}

/// One effect as interleaved stereo samples.
pub fn synth(effect: SoundEffect) -> Vec<f32> {
    let rate = RATE as f32;
    let length = match effect {
        SoundEffect::Whoosh => 0.55,
        SoundEffect::Pop => 0.16,
        SoundEffect::Tap => 0.1,
        SoundEffect::Rise => 0.8,
        SoundEffect::Chime => 1.4,
        SoundEffect::Click => 0.03,
    };
    let count = (length * rate) as usize;
    let mut out = Vec::with_capacity(count * 2);
    let mut noise = Noise(0x9e37_79b9);
    let (mut low, mut lower, mut phase) = (0f32, 0f32, 0f32);
    for n in 0..count {
        let t = n as f32 / rate;
        let p = t / length;
        let (left, right) = match effect {
            // Band-passed noise sweeping up and back, panning left to right.
            SoundEffect::Whoosh => {
                let cutoff = 300. + 3800. * (PI * p).sin();
                let a = 1. - (-TAU * cutoff / rate).exp();
                let b = 1. - (-TAU * cutoff * 0.3 / rate).exp();
                let x = noise.next();
                low += a * (x - low);
                lower += b * (x - lower);
                let s = (low - lower) * (PI * p).sin().powi(2) * 1.6;
                (s * (1. - p * 0.8), s * (0.2 + p * 0.8))
            }
            // A sine dropping in pitch, with a little click on top.
            SoundEffect::Pop => {
                phase += TAU * (250. + 950. * (-t * 14.).exp()) / rate;
                let click = if t < 0.004 { noise.next() * 0.3 } else { 0. };
                let s = phase.sin() * (-t * 28.).exp() * 0.8 + click;
                (s, s)
            }
            // A bright, short tick.
            SoundEffect::Tap => {
                let x = noise.next();
                low += 0.5 * (x - low);
                let s = (x - low) * (-t * 90.).exp() * 0.5 + (TAU * 2200. * t).sin() * (-t * 55.).exp() * 0.4;
                (s, s)
            }
            // A swelling tone into a zoom.
            SoundEffect::Rise => {
                phase += TAU * (180. + 700. * p * p) / rate;
                let swell = p * p * (1. - ((p - 0.9) / 0.1).clamp(0., 1.));
                let s = (phase.sin() + 0.3 * (2. * phase).sin() + noise.next() * 0.15) * swell * 0.5;
                (s, s)
            }
            // Three partials ringing out.
            SoundEffect::Chime => {
                let attack = (t / 0.005).min(1.);
                let s = ((TAU * 880. * t).sin() + 0.6 * (TAU * 1320. * t).sin() + 0.3 * (TAU * 1760. * t).sin())
                    * (-t * 3.2).exp()
                    * attack
                    * 0.35;
                (s, s)
            }
            SoundEffect::Click => {
                let s = noise.next() * (-t * 300.).exp() * 0.4;
                (s, s)
            }
        };
        out.push(left);
        out.push(right);
    }
    out
}

/// Any audio file macOS reads (mp3, m4a, wav, aiff, caf…) as 48 kHz
/// interleaved stereo, converted with the system's `afconvert`.
pub fn load(path: &Path) -> Result<Vec<f32>> {
    let temp = std::env::temp_dir().join(format!("appdoctor-audio-{}-{}.wav", std::process::id(), fastrand_id()));
    let status = Command::new("afconvert")
        .args(["-f", "WAVE", "-d", "LEI16@48000", "-c", "2"])
        .arg(path)
        .arg(&temp)
        .status()
        .map_err(|error| Error::Writer(format!("couldn't run afconvert: {error}")))?;
    if !status.success() {
        return Err(Error::Writer(format!("couldn't read the sound {}", path.display())));
    }
    let bytes = std::fs::read(&temp)?;
    let _ = std::fs::remove_file(&temp);
    let data = wav_data(&bytes).ok_or_else(|| Error::Writer(format!("{} didn't convert", path.display())))?;
    Ok(data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair) as f32 / 32768.)
        .collect())
}

fn fastrand_id() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// The `data` chunk of a WAV file.
fn wav_data(bytes: &[u8]) -> Option<&[u8]> {
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        if id == b"data" {
            return bytes.get(at + 8..(at + 8 + size).min(bytes.len()));
        }
        at += 8 + size + size % 2;
    }
    None
}

/// The design's sound for `length` seconds, interleaved stereo.
pub fn mix(design: &Design, images_dir: &Path, length: f32) -> Result<Vec<f32>> {
    let frames = (length * RATE as f32).ceil() as usize;
    let mut out = vec![0f32; frames * 2];
    let Some(video) = &design.video else {
        return Ok(out);
    };
    let place = |samples: &[f32], at: f32, volume: f32, fade: f32, out: &mut Vec<f32>| {
        let start = (at.max(0.) * RATE as f32) as usize;
        let count = samples.len() / 2;
        let fade = (fade.max(0.) * RATE as f32) as usize;
        // Anything still playing at the video's end fades over its last second.
        let tail = RATE as usize;
        for n in 0..count {
            let frame = start + n;
            if frame >= frames {
                break;
            }
            let mut gain = volume;
            if fade > 0 {
                gain *= (n as f32 / fade as f32).min(1.) * ((count - n) as f32 / fade as f32).min(1.);
            }
            if start + count > frames && frames - frame < tail {
                gain *= (frames - frame) as f32 / tail as f32;
            }
            out[frame * 2] += samples[n * 2] * gain;
            out[frame * 2 + 1] += samples[n * 2 + 1] * gain;
        }
    };
    for (at, effect, volume) in motion::sound_cues(&design.slides, video) {
        place(&synth(effect), at, volume, 0., &mut out);
    }
    for sound in &video.audio {
        let samples = match (&sound.effect, &sound.file) {
            (Some(effect), _) => synth(*effect),
            (None, Some(file)) => load(&images_dir.join(file))?,
            (None, None) => continue,
        };
        let from = ((sound.from.max(0.) * RATE as f32) as usize * 2).min(samples.len());
        let until = sound
            .length
            .map_or(samples.len(), |length| (from + (length.max(0.) * RATE as f32) as usize * 2).min(samples.len()));
        place(&samples[from..until], sound.at, sound.volume.clamp(0., 2.), sound.fade, &mut out);
    }
    // A soft limiter instead of clipping where sounds pile up.
    let drive = 1.1f32;
    for sample in &mut out {
        *sample = (*sample * drive).tanh() / drive.tanh();
    }
    Ok(out)
}

/// 16-bit stereo WAV.
pub fn write_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|s| ((s.clamp(-1., 1.) * 32767.) as i16).to_le_bytes())
        .collect();
    let mut wav = Vec::with_capacity(data.len() + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    std::fs::write(path, wav)?;
    Ok(())
}

/// Encodes `wav` to AAC at 256 kbps in an .m4a, as Apple's preview spec
/// asks, with the system's `afconvert`.
pub fn encode_aac(wav: &Path, m4a: &Path) -> Result<()> {
    let _ = std::fs::remove_file(m4a);
    let status = Command::new("afconvert")
        .args(["-f", "m4af", "-d", "aac", "-b", "256000"])
        .arg(wav)
        .arg(m4a)
        .status()
        .map_err(|error| Error::Writer(format!("couldn't run afconvert: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Writer("couldn't encode the sound".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_are_short_and_never_clip() {
        for effect in SoundEffect::ALL {
            let samples = synth(effect);
            assert!(!samples.is_empty() && samples.len().is_multiple_of(2));
            let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.05 && peak < 2., "{effect:?}: {peak}");
        }
    }

    #[test]
    fn wav_round_trips_through_afconvert() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("appdoctor-wav-{}.wav", std::process::id()));
        let tone: Vec<f32> = (0..RATE as usize).flat_map(|n| {
            let s = (TAU * 440. * n as f32 / RATE as f32).sin() * 0.5;
            [s, s]
        }).collect();
        write_wav(&path, &tone).unwrap();
        let back = load(&path).unwrap();
        assert!((back.len() as i64 - tone.len() as i64).abs() < 200);
        let m4a = path.with_extension("m4a");
        encode_aac(&path, &m4a).unwrap();
        assert!(std::fs::metadata(&m4a).unwrap().len() > 1000);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(m4a);
    }
}
