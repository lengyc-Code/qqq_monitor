//! Cues for quick commands, played from memory without waiting for them: a chime embedded in
//! the executable when a run starts, and a short falling chiptune pair, synthesized once,
//! when it fails.

use qqq_domain::CommandSound;
use std::sync::OnceLock;
use windows::{
    Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT},
    core::PCWSTR,
};

/// WAV image of the chime that a started run plays.
const STARTED: &[u8] = include_bytes!("../assets/command_started.wav");

/// Samples per second of the synthesized cue.
const RATE: u32 = 22_050;
/// Peak level of the synthesized cue, as a share of full scale.
const VOLUME: f32 = 0.16;
/// Rise at the start of each note, in samples, so that it starts without a click.
const ATTACK: u32 = RATE / 500;
/// Notes of the failure cue as frequency in hertz and length in milliseconds: E5, then B4.
const FAILED_NOTES: [(f32, u32); 2] = [(659.3, 110), (493.9, 220)];

/// A 16-bit mono PCM WAV image of square-wave notes, each fading out to silence.
fn wave(notes: &[(f32, u32)]) -> Vec<u8> {
    let mut samples = Vec::new();
    for &(frequency, ms) in notes {
        let count = RATE * ms / 1000;
        for n in 0..count {
            let phase = (n as f32 * frequency / RATE as f32).fract();
            let square = if phase < 0.5 { 1.0 } else { -1.0 };
            let envelope = (n as f32 / ATTACK as f32).min(1.0) * (1.0 - n as f32 / count as f32);
            samples.push((square * envelope * VOLUME * f32::from(i16::MAX)) as i16);
        }
    }
    let data = (samples.len() * 2) as u32;
    let mut image = Vec::with_capacity(44 + data as usize);
    image.extend_from_slice(b"RIFF");
    image.extend_from_slice(&(36 + data).to_le_bytes());
    image.extend_from_slice(b"WAVEfmt ");
    // Format chunk: 16 bytes of PCM, one channel, the rate, two bytes a sample, 16 bits.
    image.extend_from_slice(&16_u32.to_le_bytes());
    image.extend_from_slice(&1_u16.to_le_bytes());
    image.extend_from_slice(&1_u16.to_le_bytes());
    image.extend_from_slice(&RATE.to_le_bytes());
    image.extend_from_slice(&(RATE * 2).to_le_bytes());
    image.extend_from_slice(&2_u16.to_le_bytes());
    image.extend_from_slice(&16_u16.to_le_bytes());
    image.extend_from_slice(b"data");
    image.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        image.extend_from_slice(&sample.to_le_bytes());
    }
    image
}

/// Plays a cue and returns at once; a new cue replaces one still playing.
pub fn play(sound: CommandSound) {
    static FAILED: OnceLock<Vec<u8>> = OnceLock::new();
    let image: &[u8] = match sound {
        CommandSound::Started => STARTED,
        CommandSound::Failed => FAILED.get_or_init(|| wave(&FAILED_NOTES)),
    };
    // SAFETY: SND_MEMORY reads the WAV image the pointer names; both images are static, so
    // they outlive the asynchronous playback.
    let _ = unsafe {
        PlaySoundW(
            PCWSTR(image.as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(image: &[u8]) -> Vec<i16> {
        image[44..]
            .chunks(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    }

    /// The chunks of a RIFF WAVE image by identity, with their contents.
    fn chunks(image: &[u8]) -> Vec<(&[u8], &[u8])> {
        assert_eq!(&image[..4], b"RIFF");
        assert_eq!(&image[8..12], b"WAVE");
        let size = |at: usize| u32::from_le_bytes(image[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(size(4), image.len() - 8);
        let mut found = Vec::new();
        let mut at = 12;
        while at + 8 <= image.len() {
            let length = size(at + 4);
            found.push((&image[at..at + 4], &image[at + 8..at + 8 + length]));
            at += 8 + length + (length & 1);
        }
        assert_eq!(at, image.len());
        found
    }

    #[test]
    fn the_started_chime_is_a_short_pcm_wav() {
        let chunks = chunks(STARTED);
        let format = chunks.iter().find(|(id, _)| *id == b"fmt ").unwrap().1;
        let field = |at: usize| u16::from_le_bytes([format[at], format[at + 1]]);
        // PCM, which every system plays without a codec.
        assert_eq!(field(0), 1);
        let rate = u32::from_le_bytes(format[4..8].try_into().unwrap());
        let frame = u32::from(field(12));
        let data = chunks.iter().find(|(id, _)| *id == b"data").unwrap().1;
        let seconds = data.len() as f32 / (rate * frame) as f32;
        assert!((0.1..=2.0).contains(&seconds), "{seconds} s");
    }

    #[test]
    fn the_failure_cue_is_a_pcm_wav_image_of_its_notes() {
        let image = wave(&FAILED_NOTES);
        let length: u32 = FAILED_NOTES.iter().map(|(_, ms)| RATE * ms / 1000).sum();
        let chunks = chunks(&image);
        assert_eq!(chunks[0].0, b"fmt ");
        assert_eq!(chunks[1].0, b"data");
        assert_eq!(u32::from_le_bytes(image[24..28].try_into().unwrap()), RATE);
        assert_eq!(chunks[1].1.len() as u32, length * 2);
        assert_eq!(samples(&image).len() as u32, length);
    }

    #[test]
    fn the_failure_cue_stays_quiet_and_every_note_starts_and_ends_near_silence() {
        let samples = samples(&wave(&FAILED_NOTES));
        let limit = (VOLUME * f32::from(i16::MAX)) as i16 + 1;
        assert!(samples.iter().all(|s| s.abs() <= limit));
        let mut start = 0;
        for (_, ms) in FAILED_NOTES {
            let count = (RATE * ms / 1000) as usize;
            assert!(samples[start].abs() <= limit / 20);
            assert!(samples[start + count - 1].abs() <= limit / 20);
            start += count;
        }
    }
}
