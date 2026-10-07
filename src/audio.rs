//! Sons synthétisés en mémoire (aucun fichier audio) : cors, tambours, chocs.

use bevy::audio::Volume;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

const RATE: u32 = 22_050;

#[derive(Clone, Copy)]
pub enum Sfx {
    Jump,
    Land,
    Pickup,
    Clang,
    Death,
    Horn,
    Drum,
    Thunder,
}

#[derive(Resource)]
pub struct SfxBank {
    handles: Vec<Handle<AudioSource>>,
}

impl SfxBank {
    pub fn play(&self, commands: &mut Commands, sfx: Sfx, volume: f32) {
        let handle = self.handles[sfx as usize].clone();
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
        ));
    }
}

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, build_bank);
    }
}

fn build_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let sounds = [
        jump(),
        land(),
        pickup(),
        clang(),
        death(),
        horn(1.3, 98.0),
        drum(),
        thunder(),
    ];
    let handles = sounds
        .iter()
        .map(|s| {
            sources.add(AudioSource {
                bytes: Arc::from(wav_bytes(s)),
            })
        })
        .collect();
    commands.insert_resource(SfxBank { handles });
}

// ---------- synthèse ----------

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        // xorshift32
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn render(dur: f32, mut f: impl FnMut(f32, &mut Noise) -> f32) -> Vec<f32> {
    let n = (dur * RATE as f32) as usize;
    let mut noise = Noise(0x9E37_79B9);
    (0..n)
        .map(|i| f(i as f32 / RATE as f32, &mut noise).clamp(-1.0, 1.0))
        .collect()
}

/// Son de cor : harmoniques décroissantes, vibrato léger.
fn horn_wave(t: f32, base: f32) -> f32 {
    let vib = 1.0 + 0.012 * (TAU * 5.5 * t).sin();
    let f = base * vib;
    (1..=7)
        .map(|k| (TAU * f * k as f32 * t).sin() / (k as f32 * k as f32).sqrt())
        .sum::<f32>()
        / 3.0
}

fn env_adsr(t: f32, a: f32, sustain_until: f32, end: f32) -> f32 {
    if t < a {
        t / a
    } else if t < sustain_until {
        1.0
    } else {
        ((end - t) / (end - sustain_until)).max(0.0)
    }
}

fn horn(dur: f32, base: f32) -> Vec<f32> {
    render(dur, |t, _| {
        let e = env_adsr(t, 0.12, dur * 0.55, dur);
        0.45 * e * (horn_wave(t, base) + 0.5 * horn_wave(t, base * 1.5))
    })
}

fn jump() -> Vec<f32> {
    let d = 0.22;
    render(d, |t, n| {
        let e = (1.0 - t / d).powi(2);
        let phase = TAU * (260.0 * t + 1400.0 * t * t);
        0.5 * e * (phase.sin() * 0.7 + n.next() * 0.3)
    })
}

fn land() -> Vec<f32> {
    let d = 0.2;
    render(d, |t, n| {
        let e = (1.0 - t / d).powf(1.5);
        let phase = TAU * (140.0 * t - 200.0 * t * t);
        let click = if t < 0.02 { n.next() * 0.6 } else { 0.0 };
        0.7 * e * phase.sin() + click
    })
}

fn pickup() -> Vec<f32> {
    let d = 0.55;
    render(d, |t, _| {
        let mut s = 0.0;
        for (i, f) in [660.0_f32, 880.0, 1320.0, 1760.0].iter().enumerate() {
            let t0 = i as f32 * 0.09;
            if t >= t0 {
                let lt = t - t0;
                let e = (-lt * 9.0).exp();
                s += e * ((TAU * f * lt).sin() + 0.3 * (TAU * f * 2.0 * lt).sin());
            }
        }
        0.3 * s
    })
}

fn clang() -> Vec<f32> {
    let d = 0.6;
    render(d, |t, n| {
        let mut s = 0.0;
        for (f, k) in [(820.0_f32, 7.0_f32), (1290.0, 9.0), (2070.0, 12.0), (3310.0, 16.0)] {
            s += (-t * k).exp() * (TAU * f * t).sin();
        }
        let burst = if t < 0.03 { n.next() * (1.0 - t / 0.03) } else { 0.0 };
        0.35 * s + 0.5 * burst
    })
}

fn death() -> Vec<f32> {
    let d = 1.8;
    render(d, |t, n| {
        // Cor funèbre qui descend d'une tierce, puis tambour.
        let base = 110.0 * (1.0 - 0.17 * (t / d).powf(0.6));
        let e = env_adsr(t, 0.05, 1.0, d);
        let horn = 0.4 * e * horn_wave(t, base);
        let drum = if t > 0.6 {
            let lt = t - 0.6;
            (-lt * 10.0).exp() * (TAU * (120.0 * lt - 150.0 * lt * lt)).sin() * 0.8
                + (-lt * 40.0).exp() * n.next() * 0.3
        } else {
            0.0
        };
        horn + drum
    })
}

fn drum() -> Vec<f32> {
    let d = 0.35;
    render(d, |t, n| {
        (-t * 11.0).exp() * (TAU * (150.0 * t - 180.0 * t * t)).sin() * 0.9
            + (-t * 45.0).exp() * n.next() * 0.4
    })
}

fn thunder() -> Vec<f32> {
    let d = 1.6;
    let mut lp = 0.0;
    render(d, |t, n| {
        // Bruit filtré passe-bas avec grondement irrégulier.
        let raw = n.next();
        lp += (raw - lp) * 0.08;
        let rumble = 1.0 + 0.6 * (TAU * 3.0 * t + 2.0 * (TAU * 0.7 * t).sin()).sin();
        let e = if t < 0.05 { t / 0.05 } else { (-(t - 0.05) * 2.2).exp() };
        (0.9 * e * rumble * lp * 2.5 + 0.2 * e * (PI * 55.0 * t).sin()).clamp(-1.0, 1.0)
    })
}

/// Encode des échantillons [-1, 1] en WAV PCM 16 bits mono.
fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}
