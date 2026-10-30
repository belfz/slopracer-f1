//! A tiny 8-bit style synthesizer: a pulse-wave engine drone plus one-shot
//! square/triangle/noise voices for effects. Silently disabled if no audio
//! device is available.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crate::music::{Music, Tune};
use cpal::{FromSample, SizedSample};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
pub enum Sfx {
    Light,
    Go,
    Bump,
    Crash,
    Lap,
    Finish,
    Shift,
    Blip,
    Select,
    /// BIOS typing click.
    Tick,
}

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Noise,
}

struct Voice {
    wave: Wave,
    f0: f32,
    f1: f32,
    dur: f32,
    delay: f32,
    vol: f32,
    t: f32,
    phase: f32,
    noise: f32,
}

struct Synth {
    sr: f32,
    engine_f: f32,
    engine_target: f32,
    engine_vol: f32,
    engine_vol_target: f32,
    ph1: f32,
    ph2: f32,
    voices: Vec<Voice>,
    lfsr: u16,
    lp: f32,
    muted: bool,
    music: Music,
}

fn lfsr_step(lfsr: &mut u16) -> f32 {
    // NES-style 15-bit LFSR.
    let bit = (*lfsr ^ (*lfsr >> 1)) & 1;
    *lfsr = (*lfsr >> 1) | (bit << 14);
    if *lfsr & 1 == 1 { 1.0 } else { -1.0 }
}

impl Synth {
    fn next_sample(&mut self) -> f32 {
        let dt = 1.0 / self.sr;
        self.engine_f += (self.engine_target - self.engine_f) * 0.002;
        self.engine_vol += (self.engine_vol_target - self.engine_vol) * 0.002;

        self.ph1 = (self.ph1 + self.engine_f * dt).fract();
        self.ph2 = (self.ph2 + self.engine_f * 1.007 * dt).fract();
        let p1 = if self.ph1 < 0.25 { 1.0 } else { -1.0 };
        let p2 = if self.ph2 < 0.5 { 1.0 } else { -1.0 };
        let mut out = (p1 * 0.6 + p2 * 0.4) * self.engine_vol;

        for v in &mut self.voices {
            if v.delay > 0.0 {
                v.delay -= dt;
                continue;
            }
            if v.t >= v.dur {
                continue;
            }
            let k = v.t / v.dur;
            let f = v.f0 + (v.f1 - v.f0) * k;
            v.t += dt;
            let prev = v.phase;
            v.phase = (v.phase + f * dt).fract();
            let (s, env) = match v.wave {
                Wave::Square => (if v.phase < 0.5 { 1.0 } else { -1.0 }, (1.0 - k).powf(0.3)),
                Wave::Triangle => (4.0 * (v.phase - 0.5).abs() - 1.0, (1.0 - k).powf(0.3)),
                Wave::Noise => {
                    // Sample-and-hold noise, re-rolled at frequency `f`.
                    if v.phase < prev {
                        v.noise = lfsr_step(&mut self.lfsr);
                    }
                    (v.noise, 1.0 - k)
                }
            };
            out += s * env * v.vol;
        }

        // Gentle low-pass to take the harsh edge off the square waves.
        self.lp += (out - self.lp) * 0.3;
        let mixed = self.lp + self.music.next_sample() * 0.55;
        if self.muted {
            return 0.0;
        }
        mixed.clamp(-1.0, 1.0)
    }

    fn cleanup(&mut self) {
        self.voices.retain(|v| v.delay > 0.0 || v.t < v.dur);
    }

    fn add(&mut self, wave: Wave, f0: f32, f1: f32, dur: f32, delay: f32, vol: f32) {
        self.voices.push(Voice { wave, f0, f1, dur, delay, vol, t: 0.0, phase: 0.0, noise: 1.0 });
    }
}

pub struct Audio {
    synth: Arc<Mutex<Synth>>,
    _stream: Option<cpal::Stream>,
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        let synth = Arc::new(Mutex::new(Synth {
            sr: 44_100.0,
            engine_f: 50.0,
            engine_target: 50.0,
            engine_vol: 0.0,
            engine_vol_target: 0.0,
            ph1: 0.0,
            ph2: 0.0,
            voices: Vec::new(),
            lfsr: 1,
            lp: 0.0,
            muted,
            music: Music::new(44_100.0),
        }));
        let stream = open_stream(synth.clone());
        Audio { synth, _stream: stream }
    }

    pub fn available(&self) -> bool {
        self._stream.is_some()
    }

    pub fn toggle_mute(&self) -> bool {
        let mut s = self.synth.lock().unwrap();
        s.muted = !s.muted;
        s.muted
    }

    /// `rpm` in 0..~1.1; `on` false silences the engine.
    pub fn set_engine(&self, rpm: f32, throttle: bool, on: bool) {
        let mut s = self.synth.lock().unwrap();
        s.engine_target = 45.0 + rpm.max(0.0) * 200.0;
        s.engine_vol_target = if on { if throttle { 0.10 } else { 0.06 } } else { 0.0 };
    }

    /// Play a track's soundtrack (or none) at volume `vol`, with fades.
    pub fn set_music(&self, tune: Option<Tune>, vol: f32) {
        self.synth.lock().unwrap().music.set(tune, vol);
    }

    pub fn play(&self, sfx: Sfx) {
        let mut s = self.synth.lock().unwrap();
        match sfx {
            Sfx::Light => s.add(Wave::Square, 440.0, 440.0, 0.3, 0.0, 0.18),
            Sfx::Go => s.add(Wave::Square, 880.0, 880.0, 0.7, 0.0, 0.2),
            Sfx::Bump => s.add(Wave::Noise, 900.0, 200.0, 0.18, 0.0, 0.35),
            Sfx::Crash => {
                s.add(Wave::Noise, 3000.0, 60.0, 1.1, 0.0, 0.5);
                s.add(Wave::Square, 220.0, 40.0, 0.5, 0.0, 0.15);
            }
            Sfx::Lap => {
                for (i, f) in [523.0, 659.0, 784.0].into_iter().enumerate() {
                    s.add(Wave::Square, f, f, 0.12, i as f32 * 0.1, 0.15);
                }
            }
            Sfx::Finish => {
                let notes = [(523.0, 0.12), (659.0, 0.12), (784.0, 0.12), (1047.0, 0.3), (784.0, 0.12), (1047.0, 0.6)];
                let mut t = 0.0;
                for (f, d) in notes {
                    s.add(Wave::Square, f, f, d, t, 0.16);
                    s.add(Wave::Triangle, f / 2.0, f / 2.0, d, t, 0.25);
                    t += d + 0.03;
                }
            }
            Sfx::Shift => s.add(Wave::Triangle, 300.0, 600.0, 0.06, 0.0, 0.3),
            Sfx::Blip => s.add(Wave::Square, 660.0, 660.0, 0.04, 0.0, 0.12),
            Sfx::Select => {
                s.add(Wave::Square, 660.0, 660.0, 0.06, 0.0, 0.14);
                s.add(Wave::Square, 990.0, 990.0, 0.12, 0.06, 0.14);
            }
            Sfx::Tick => s.add(Wave::Square, 1800.0, 1800.0, 0.012, 0.0, 0.05),
        }
    }
}

fn open_stream(synth: Arc<Mutex<Synth>>) -> Option<cpal::Stream> {
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    let format = config.sample_format();
    let cfg: cpal::StreamConfig = config.into();
    {
        let mut s = synth.lock().unwrap();
        let sr = cfg.sample_rate.0 as f32;
        s.sr = sr;
        s.music.set_rate(sr);
    }
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &cfg, synth),
        cpal::SampleFormat::I16 => build::<i16>(&device, &cfg, synth),
        cpal::SampleFormat::U16 => build::<u16>(&device, &cfg, synth),
        _ => None,
    }?;
    stream.play().ok()?;
    Some(stream)
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    synth: Arc<Mutex<Synth>>,
) -> Option<cpal::Stream> {
    let channels = cfg.channels as usize;
    device
        .build_output_stream(
            cfg,
            move |data: &mut [T], _| {
                if let Ok(mut s) = synth.try_lock() {
                    for frame in data.chunks_mut(channels) {
                        let v = T::from_sample(s.next_sample());
                        frame.iter_mut().for_each(|x| *x = v);
                    }
                    s.cleanup();
                } else {
                    data.iter_mut().for_each(|x| *x = T::from_sample(0.0f32));
                }
            },
            |_err| {},
            None,
        )
        .ok()
}
