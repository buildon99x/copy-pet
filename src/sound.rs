//! Synthesized sound effects, generated in memory at startup.
//!
//! The WAV bytes are pure data, built once here in the platform-agnostic
//! core. Actually playing a buffer is an OS call, so this module never makes
//! one itself: the backend registers a player via [`set_player`] at startup
//! (Windows plays through winmm `PlaySound`, SND_MEMORY | SND_ASYNC — no
//! audio assets, no extra dependencies). Other platforms never register one,
//! so playback stays a silent no-op for now (see ADR-0002); a portable audio
//! backend can register its own player behind the same hook.

use std::sync::OnceLock;

const RATE: u32 = 22050;

fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + samples.len() * 2);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&RATE.to_le_bytes());
    v.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    v.extend_from_slice(&2u16.to_le_bytes()); // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/// Soft, short "tap" — sine with fast exponential decay plus a hint of
/// second harmonic so it sounds like a felt paw, not a beep.
fn tap(freq: f32) -> Vec<u8> {
    let n = (RATE as f32 * 0.045) as usize;
    let mut s = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let env = (-t * 110.0).exp();
        let v = (t * freq * std::f32::consts::TAU).sin() * 0.8
            + (t * freq * 2.0 * std::f32::consts::TAU).sin() * 0.2;
        s.push((v * env * 0.35 * i16::MAX as f32) as i16);
    }
    wav(&s)
}

/// Three ascending notes for level-up.
fn chime() -> Vec<u8> {
    let notes = [523.25f32, 659.25, 783.99]; // C5 E5 G5
    let per = (RATE as f32 * 0.11) as usize;
    let mut s = Vec::with_capacity(per * notes.len());
    for (ni, f) in notes.iter().enumerate() {
        for i in 0..per {
            let t = i as f32 / RATE as f32;
            let env = (-t * 14.0).exp() * (1.0 - (i as f32 / per as f32).powi(4));
            let v = (t * f * std::f32::consts::TAU).sin();
            let tail = if ni == notes.len() - 1 { 1.4 } else { 1.0 };
            s.push((v * env * 0.30 * tail * i16::MAX as f32) as i16);
        }
    }
    wav(&s)
}

/// Quick upward "boop" for petting.
fn pop() -> Vec<u8> {
    let n = (RATE as f32 * 0.09) as usize;
    let mut s = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let f = 500.0 + 600.0 * (t / 0.09);
        let env = (-t * 35.0).exp();
        s.push(((t * f * std::f32::consts::TAU).sin() * env * 0.3 * i16::MAX as f32) as i16);
    }
    wav(&s)
}

/// Two quick low "bites" with a pitch drop — the fish nom.
fn nom() -> Vec<u8> {
    let bite = |f0: f32, s: &mut Vec<i16>| {
        let n = (RATE as f32 * 0.05) as usize;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let f = f0 * (1.0 - t * 4.0);
            let env = (-t * 90.0).exp();
            let v = (t * f * std::f32::consts::TAU).sin() * 0.7
                + (t * f * 2.7 * std::f32::consts::TAU).sin() * 0.3;
            s.push((v * env * 0.4 * i16::MAX as f32) as i16);
        }
    };
    let mut s = Vec::new();
    bite(340.0, &mut s);
    s.extend(std::iter::repeat_n(0i16, (RATE as f32 * 0.035) as usize));
    bite(270.0, &mut s);
    wav(&s)
}

static TAP_L: OnceLock<Vec<u8>> = OnceLock::new();
static TAP_R: OnceLock<Vec<u8>> = OnceLock::new();
static CHIME: OnceLock<Vec<u8>> = OnceLock::new();
static POP: OnceLock<Vec<u8>> = OnceLock::new();
static NOM: OnceLock<Vec<u8>> = OnceLock::new();

/// The backend's sound-playback hook, registered once via [`set_player`].
static PLAYER: OnceLock<fn(&'static [u8])> = OnceLock::new();

/// Registers the backend's OS sound-playback function. Call once at startup,
/// before [`init`]; later calls are ignored (first write wins). Backends
/// that don't register one leave playback a no-op.
pub fn set_player(f: fn(&'static [u8])) {
    let _ = PLAYER.set(f);
}

pub fn init() {
    TAP_L.get_or_init(|| tap(880.0));
    TAP_R.get_or_init(|| tap(660.0));
    CHIME.get_or_init(chime);
    POP.get_or_init(pop);
    NOM.get_or_init(nom);
}

fn play(data: &'static [u8]) {
    if let Some(player) = PLAYER.get() {
        player(data);
    }
}

pub fn play_tap(left: bool) {
    let s: Option<&'static Vec<u8>> = if left { TAP_L.get() } else { TAP_R.get() };
    if let Some(s) = s {
        play(s);
    }
}

pub fn play_chime() {
    if let Some(s) = CHIME.get() {
        play(s);
    }
}

pub fn play_pop() {
    if let Some(s) = POP.get() {
        play(s);
    }
}

/// Crunchy double-bite when the cat eats a copy-event fish.
pub fn play_nom() {
    if let Some(s) = NOM.get() {
        play(s);
    }
}
