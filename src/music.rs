//! Track soundtracks: a tiny synthwave sequencer with band-limited saw
//! waves, a four-on-the-floor drum kit, sidechain pumping and a tempo-synced
//! echo. Runs inside the audio callback, one sample at a time.

use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tune {
    NeonSakura,
}

const BPM: f32 = 104.0;
const STEPS_PER_BAR: usize = 16;
/// Two intro bars, then a 16-bar loop: 8 bars of arpeggio, 8 with the lead.
const INTRO_BARS: usize = 2;
const LOOP_BARS: usize = 16;

/// A minor: Am – F – C – G, as (bass root, triad) MIDI notes.
const CHORDS: [(u8, [u8; 3]); 4] = [(45, [57, 60, 64]), (41, [57, 60, 65]), (48, [55, 60, 64]), (43, [55, 59, 62])];

/// Lead melody over 8 bars: (bar, step, MIDI note, length in steps).
const MELODY: &[(u8, u8, u8, u8)] = &[
    (0, 0, 76, 6), (0, 6, 74, 2), (0, 8, 72, 4), (0, 12, 74, 4),
    (1, 0, 72, 6), (1, 6, 69, 2), (1, 8, 72, 8),
    (2, 0, 67, 4), (2, 4, 72, 4), (2, 8, 76, 6), (2, 14, 79, 2),
    (3, 0, 74, 8), (3, 8, 71, 4), (3, 12, 74, 4),
    (4, 0, 76, 4), (4, 4, 81, 6), (4, 10, 79, 2), (4, 12, 76, 4),
    (5, 0, 77, 6), (5, 6, 76, 2), (5, 8, 72, 8),
    (6, 0, 76, 4), (6, 4, 79, 4), (6, 8, 84, 4), (6, 12, 83, 4),
    (7, 0, 79, 6), (7, 6, 77, 2), (7, 8, 76, 4), (7, 12, 74, 4),
];

fn midi(n: u8) -> f32 {
    440.0 * 2f32.powf((n as f32 - 69.0) / 12.0)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Bass,
    Pad,
    Arp,
    Lead,
    Kick,
    Snare,
    Hat,
}

struct Voice {
    kind: Kind,
    freq: f32,
    t: f32,
    gate: f32,
    vol: f32,
    ph: [f32; 3],
    lp: [f32; 2],
}

impl Voice {
    fn release(&self) -> f32 {
        match self.kind {
            Kind::Pad => 0.8,
            Kind::Lead => 0.25,
            Kind::Bass | Kind::Arp => 0.06,
            Kind::Kick | Kind::Snare | Kind::Hat => 0.0,
        }
    }

    fn done(&self) -> bool {
        match self.kind {
            Kind::Kick => self.t > 0.5,
            Kind::Snare => self.t > 0.4,
            Kind::Hat => self.t > 0.12,
            _ => self.t > self.gate + self.release(),
        }
    }

    fn env(&self) -> f32 {
        let (a, d, s) = match self.kind {
            Kind::Pad => (0.35, 0.5, 0.8),
            Kind::Lead => (0.02, 0.3, 0.7),
            Kind::Bass => (0.004, 0.15, 0.55),
            _ => (0.003, 0.08, 0.3),
        };
        let held = if self.t < a { self.t / a } else { s + (1.0 - s) * (-(self.t - a) / d).exp() };
        if self.t <= self.gate {
            held
        } else {
            let at_gate = if self.gate < a { self.gate / a } else { s + (1.0 - s) * (-(self.gate - a) / d).exp() };
            at_gate * (1.0 - (self.t - self.gate) / self.release()).max(0.0)
        }
    }
}

/// Band-limited sawtooth (polyBLEP).
fn saw(ph: f32, dt: f32) -> f32 {
    let mut v = 2.0 * ph - 1.0;
    if ph < dt {
        let t = ph / dt;
        v -= t + t - t * t - 1.0;
    } else if ph > 1.0 - dt {
        let t = (ph - 1.0) / dt;
        v -= t * t + t + t + 1.0;
    }
    v
}

fn lp_coef(fc: f32, sr: f32) -> f32 {
    1.0 - (-TAU * fc.min(sr * 0.45) / sr).exp()
}

pub struct Music {
    sr: f32,
    tune: Option<Tune>,
    vol: f32,
    vol_target: f32,
    /// Samples into the current step.
    clock: f32,
    step: usize,
    since_kick: f32,
    voices: Vec<Voice>,
    echo: Vec<f32>,
    echo_pos: usize,
    rng: u32,
    hat_lp: f32,
}

impl Music {
    pub fn new(sr: f32) -> Music {
        let mut m = Music {
            sr,
            tune: None,
            vol: 0.0,
            vol_target: 0.0,
            clock: 0.0,
            step: 0,
            since_kick: 1.0,
            voices: Vec::new(),
            echo: Vec::new(),
            echo_pos: 0,
            rng: 0x1234_5678,
            hat_lp: 0.0,
        };
        m.set_rate(sr);
        m
    }

    pub fn set_rate(&mut self, sr: f32) {
        self.sr = sr;
        // Dotted-eighth echo.
        let len = (60.0 / BPM * 0.75 * sr) as usize;
        self.echo = vec![0.0; len.max(1)];
        self.echo_pos = 0;
    }

    /// Select a tune and fade to `vol`. A different tune first fades the
    /// current one out, then starts from the top.
    pub fn set(&mut self, tune: Option<Tune>, vol: f32) {
        if tune == self.tune {
            self.vol_target = if tune.is_some() { vol } else { 0.0 };
            return;
        }
        if self.tune.is_some() && self.vol > 0.01 {
            self.vol_target = 0.0;
            return;
        }
        self.tune = tune;
        self.step = 0;
        self.clock = 0.0;
        self.voices.clear();
        self.echo.iter_mut().for_each(|x| *x = 0.0);
        self.vol_target = if tune.is_some() { vol } else { 0.0 };
    }

    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn play(&mut self, kind: Kind, note: u8, steps: f32, vol: f32) {
        let gate = steps * 60.0 / BPM / 4.0;
        let freq = if note == 0 { 0.0 } else { midi(note) };
        self.voices.push(Voice { kind, freq, t: 0.0, gate, vol, ph: [0.0, 0.33, 0.66], lp: [0.0; 2] });
    }

    fn trigger_step(&mut self) {
        let bar_abs = self.step / STEPS_PER_BAR;
        let st = self.step % STEPS_PER_BAR;
        let intro = bar_abs < INTRO_BARS;
        let bar = if intro { bar_abs } else { (bar_abs - INTRO_BARS) % LOOP_BARS };
        let (root, triad) = CHORDS[bar % 4];
        let with_lead = !intro && bar >= 8;

        if st == 0 {
            for n in triad {
                self.play(Kind::Pad, n, 15.5, 0.05);
            }
            self.play(Kind::Pad, root + 12, 15.5, 0.04);
        }
        if !intro {
            if st.is_multiple_of(4) {
                self.play(Kind::Kick, 0, 1.0, 0.75);
                self.since_kick = 0.0;
            }
            if st == 4 || st == 12 {
                self.play(Kind::Snare, 0, 1.0, 0.32);
            }
            if st.is_multiple_of(2) {
                let note = if st.is_multiple_of(4) { root } else { root + 12 };
                self.play(Kind::Bass, note, 1.6, 0.24);
            }
            let hat = if st % 4 == 2 { 0.09 } else { 0.035 };
            self.play(Kind::Hat, 0, 1.0, hat);
        }
        // Rising-and-falling arpeggio over the chord.
        const ARP: [usize; 8] = [0, 1, 2, 3, 2, 1, 2, 3];
        let tones = [triad[0] + 12, triad[1] + 12, triad[2] + 12, triad[0] + 24];
        let arp_vol = if with_lead { 0.025 } else { 0.05 };
        self.play(Kind::Arp, tones[ARP[st % 8]], 0.8, arp_vol);

        if with_lead {
            let lb = (bar - 8) as u8;
            let notes: Vec<_> = MELODY.iter().filter(|m| m.0 == lb && m.1 as usize == st).copied().collect();
            for (_, _, n, len) in notes {
                self.play(Kind::Lead, n, len as f32 - 0.3, 0.11);
            }
        }
        self.step = (self.step + 1) % ((INTRO_BARS + LOOP_BARS) * STEPS_PER_BAR);
        if self.step == 0 {
            self.step = INTRO_BARS * STEPS_PER_BAR;
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        self.vol += (self.vol_target - self.vol) * 0.0004;
        if self.tune.is_none() {
            return 0.0;
        }
        if self.vol_target == 0.0 && self.vol < 1e-4 {
            // Faded out: stop, so a different tune can start.
            self.tune = None;
            self.voices.clear();
            return 0.0;
        }
        let step_len = 60.0 / BPM / 4.0 * self.sr;
        if self.clock <= 0.0 {
            self.trigger_step();
            self.clock += step_len;
        }
        self.clock -= 1.0;

        let dt = 1.0 / self.sr;
        self.since_kick += dt;
        let pump = 1.0 - 0.55 * (-self.since_kick * 9.0).exp();
        let (mut dry, mut send) = (0.0f32, 0.0f32);
        let mut noise_needed = 0;
        for v in &self.voices {
            if matches!(v.kind, Kind::Snare | Kind::Hat) {
                noise_needed += 1;
            }
        }
        let noises: Vec<f32> = (0..noise_needed).map(|_| self.noise()).collect();
        let mut ni = 0;
        let sr = self.sr;
        for v in &mut self.voices {
            let env = v.env();
            let out = match v.kind {
                Kind::Bass => {
                    let fdt = v.freq * dt;
                    v.ph[0] = (v.ph[0] + fdt).fract();
                    let raw = saw(v.ph[0], fdt);
                    // Plucky filter sweep.
                    let a = lp_coef(180.0 + 1400.0 * (-v.t * 14.0).exp(), sr);
                    v.lp[0] += (raw - v.lp[0]) * a;
                    v.lp[1] += (v.lp[0] - v.lp[1]) * a;
                    v.lp[1] * env * v.vol * pump
                }
                Kind::Pad => {
                    let mut s = 0.0;
                    for (k, det) in [0.996f32, 1.0, 1.005].into_iter().enumerate() {
                        let fdt = v.freq * det * dt;
                        v.ph[k] = (v.ph[k] + fdt).fract();
                        s += saw(v.ph[k], fdt);
                    }
                    let a = lp_coef(1100.0, sr);
                    v.lp[0] += (s - v.lp[0]) * a;
                    v.lp[1] += (v.lp[0] - v.lp[1]) * a;
                    v.lp[1] * env * v.vol * pump
                }
                Kind::Arp => {
                    v.ph[0] = (v.ph[0] + v.freq * dt).fract();
                    let sq = if v.ph[0] < 0.5 { 1.0 } else { -1.0 };
                    let a = lp_coef(2400.0, sr);
                    v.lp[0] += (sq - v.lp[0]) * a;
                    let o = v.lp[0] * env * v.vol;
                    send += o;
                    o
                }
                Kind::Lead => {
                    let vib = 1.0 + 0.006 * (v.t * TAU * 5.5).sin() * (v.t * 2.0).min(1.0);
                    let mut s = 0.0;
                    for (k, det) in [1.0f32, 1.008].into_iter().enumerate() {
                        let fdt = v.freq * vib * det * dt;
                        v.ph[k] = (v.ph[k] + fdt).fract();
                        s += saw(v.ph[k], fdt);
                    }
                    let a = lp_coef(2600.0 + 1800.0 * (-v.t * 4.0).exp(), sr);
                    v.lp[0] += (s - v.lp[0]) * a;
                    v.lp[1] += (v.lp[0] - v.lp[1]) * a;
                    let o = v.lp[1] * env * v.vol;
                    send += o;
                    o
                }
                Kind::Kick => {
                    let f = 48.0 + 130.0 * (-v.t * 32.0).exp();
                    v.ph[0] = (v.ph[0] + f * dt).fract();
                    (v.ph[0] * TAU).sin() * (-v.t * 7.0).exp() * v.vol
                }
                Kind::Snare => {
                    let n = noises[ni];
                    ni += 1;
                    v.ph[0] = (v.ph[0] + 185.0 * dt).fract();
                    let body = (v.ph[0] * TAU).sin() * (-v.t * 25.0).exp() * 0.5;
                    // Big gated-reverb tail, cut off sharply.
                    let tail = if v.t < 0.28 { 0.35 * (-v.t * 4.0).exp() } else { 0.0 };
                    let o = (n * ((-v.t * 18.0).exp() * 0.7 + tail) + body) * v.vol;
                    send += o * 0.3;
                    o
                }
                Kind::Hat => {
                    let n = noises[ni];
                    ni += 1;
                    n * (-v.t * 55.0).exp() * v.vol
                }
            };
            v.t += dt;
            dry += out;
        }
        self.voices.retain(|v| !v.done());
        // High-pass the hats' noise floor out of the low end.
        self.hat_lp += (dry - self.hat_lp) * lp_coef(25.0, sr);
        let dry = dry - self.hat_lp;

        let echoed = self.echo[self.echo_pos];
        self.echo[self.echo_pos] = send + echoed * 0.38;
        self.echo_pos = (self.echo_pos + 1) % self.echo.len();
        let mix = dry + echoed * 0.45;
        (mix * 1.1).tanh() * self.vol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders the loop offline: it must be audible, never clip, and stay
    /// cheap enough for the audio thread. Also writes a WAV for listening.
    #[test]
    fn neon_sakura_renders_cleanly() {
        let sr = 44_100.0;
        let mut m = Music::new(sr);
        m.set(Some(Tune::NeonSakura), 1.0);
        let secs = 50.0;
        let n = (sr * secs) as usize;
        let t = std::time::Instant::now();
        let samples: Vec<f32> = (0..n).map(|_| m.next_sample()).collect();
        let cpu = t.elapsed().as_secs_f32() / secs;
        let peak = samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        let tail = &samples[n / 2..];
        let rms = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
        println!("peak {peak:.3} rms {rms:.3} cpu {:.2}% voices {}", cpu * 100.0, m.voices.len());
        assert!(samples.iter().all(|s| s.is_finite()));
        assert!(peak <= 1.0);
        assert!(rms > 0.05, "too quiet");
        assert!(m.voices.len() < 40, "voices leak");

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        let mut wav = Vec::new();
        let data_len = (n * 2) as u32;
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&(sr as u32).to_le_bytes());
        wav.extend_from_slice(&(sr as u32 * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            wav.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes());
        }
        std::fs::write(dir.join("neon_sakura.wav"), wav).unwrap();
    }

    #[test]
    fn switching_tunes_fades_and_stops() {
        let mut m = Music::new(8_000.0);
        m.set(Some(Tune::NeonSakura), 1.0);
        for _ in 0..8_000 {
            m.next_sample();
        }
        m.set(None, 0.0);
        let mut last = 1.0;
        for _ in 0..80_000 {
            last = m.next_sample();
        }
        assert!(last.abs() < 1e-3 && m.tune.is_none());
    }
}
