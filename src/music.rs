//! Track soundtracks: a tiny sequencer with band-limited saw waves, an FM
//! electric piano, bells, a drum kit, sidechain pumping and a tempo-synced
//! echo. Runs inside the audio callback, one sample at a time.
//!
//! - Neon Sakura: synthwave in A minor, four on the floor.
//! - Shirayuki: breezy 80s city pop over the "royal road" progression.

use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tune {
    NeonSakura,
    Shirayuki,
    TokyoMidnight,
}

impl Tune {
    fn bpm(self) -> f32 {
        match self {
            Tune::NeonSakura => 104.0,
            Tune::Shirayuki => 108.0,
            Tune::TokyoMidnight => 112.0,
        }
    }
}

/// Slowest tempo, which sizes the echo buffer.
const MIN_BPM: f32 = 100.0;
const STEPS_PER_BAR: usize = 16;
/// Two intro bars, then a 16-bar loop: 8 bars of arpeggio, 8 with the lead.
const INTRO_BARS: usize = 2;
const LOOP_BARS: usize = 16;
const MAX_VOICES: usize = 64;

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

/// Shirayuki: Fmaj9 – G13 – Em9 – Am9 – Dm9 – G13 – Cmaj9 – C13, as
/// (bass root, four-note voicing).
const CITY_CHORDS: [(u8, [u8; 4]); 8] = [
    (41, [57, 60, 64, 67]),
    (43, [53, 59, 64, 69]),
    (40, [55, 59, 62, 66]),
    (45, [55, 60, 64, 71]),
    (38, [53, 57, 60, 64]),
    (43, [53, 59, 64, 69]),
    (48, [55, 59, 62, 64]),
    (48, [52, 58, 62, 69]),
];

const CITY_MELODY: &[(u8, u8, u8, u8)] = &[
    (0, 0, 72, 2), (0, 2, 74, 2), (0, 4, 76, 6), (0, 10, 79, 2), (0, 12, 76, 4),
    (1, 0, 74, 6), (1, 6, 72, 2), (1, 8, 74, 4), (1, 12, 79, 4),
    (2, 0, 71, 4), (2, 4, 74, 4), (2, 8, 79, 6), (2, 14, 78, 2),
    (3, 0, 76, 8), (3, 8, 72, 4), (3, 12, 74, 4),
    (4, 0, 77, 4), (4, 4, 76, 2), (4, 6, 74, 2), (4, 8, 72, 4), (4, 12, 69, 4),
    (5, 0, 71, 6), (5, 6, 74, 2), (5, 8, 79, 8),
    (6, 0, 76, 4), (6, 4, 79, 4), (6, 8, 83, 4), (6, 12, 84, 4),
    (7, 0, 82, 6), (7, 6, 81, 2), (7, 8, 79, 4), (7, 12, 76, 4),
];

/// Bass line per bar: (step, interval above the root, length in steps).
const CITY_BASS: [(usize, u8, f32); 6] = [(0, 0, 2.6), (3, 12, 0.8), (6, 0, 1.8), (8, 7, 1.8), (11, 12, 0.8), (14, 7, 1.6)];

const MIDNIGHT_CHORDS: [(u8, [u8; 4]); 8] = [
    (42, [54, 57, 61, 64]), (38, [54, 57, 61, 66]),
    (45, [56, 61, 64, 68]), (40, [56, 59, 62, 66]),
    (35, [54, 57, 62, 66]), (38, [54, 57, 61, 64]),
    (37, [56, 59, 61, 64]), (37, [53, 59, 61, 68]),
];

const MIDNIGHT_MELODY: &[(u8, u8, u8, u8)] = &[
    (0, 2, 78, 6), (0, 10, 76, 2), (0, 12, 73, 3),
    (1, 0, 73, 6), (1, 8, 69, 3), (1, 12, 73, 3),
    (2, 2, 76, 6), (2, 10, 73, 2), (2, 12, 71, 3),
    (3, 0, 68, 6), (3, 10, 71, 5),
    (4, 2, 78, 4), (4, 8, 81, 4), (4, 12, 78, 3),
    (5, 0, 76, 6), (5, 8, 73, 7),
    (6, 2, 76, 4), (6, 8, 73, 4), (6, 12, 71, 3),
    (7, 0, 73, 6), (7, 8, 68, 7),
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
    EPiano,
    Bell,
    Pluck,
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
            Kind::EPiano => 0.3,
            Kind::Pluck => 0.18,
            Kind::Bell => 0.0,
            Kind::Bass | Kind::Arp => 0.06,
            Kind::Kick | Kind::Snare | Kind::Hat => 0.0,
        }
    }

    fn done(&self) -> bool {
        match self.kind {
            Kind::Kick => self.t > 0.5,
            Kind::Snare => self.t > 0.4,
            Kind::Hat => self.t > 0.12,
            Kind::Bell => self.t > 1.6,
            _ => self.t > self.gate + self.release(),
        }
    }

    fn env(&self) -> f32 {
        let (a, d, s) = match self.kind {
            Kind::Pad => (0.35, 0.5, 0.8),
            Kind::Lead => (0.02, 0.3, 0.7),
            Kind::Bass => (0.004, 0.15, 0.55),
            Kind::EPiano => (0.003, 0.7, 0.3),
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
    pending: Option<(Option<Tune>, f32)>,
    /// Samples into the current step.
    clock: f32,
    step: usize,
    since_kick: f32,
    voices: Vec<Voice>,
    echo: Vec<f32>,
    echo_len: usize,
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
            pending: None,
            clock: 0.0,
            step: 0,
            since_kick: 1.0,
            voices: Vec::with_capacity(MAX_VOICES),
            echo: Vec::new(),
            echo_len: 1,
            echo_pos: 0,
            rng: 0x1234_5678,
            hat_lp: 0.0,
        };
        m.set_rate(sr);
        m
    }

    pub fn set_rate(&mut self, sr: f32) {
        self.sr = sr;
        self.echo = vec![0.0; (60.0 / MIN_BPM * 0.75 * sr) as usize + 1];
        self.sync_echo();
    }

    fn bpm(&self) -> f32 {
        self.tune.map_or(120.0, Tune::bpm)
    }

    /// Dotted-eighth echo at the current tune's tempo.
    fn sync_echo(&mut self) {
        self.echo_len = ((60.0 / self.bpm() * 0.75 * self.sr) as usize).clamp(1, self.echo.len());
        self.echo_pos = 0;
    }

    /// Select a tune and fade to `vol`. A different tune first fades the
    /// current one out, then starts from the top.
    pub fn set(&mut self, tune: Option<Tune>, vol: f32) {
        if tune == self.tune {
            self.pending = None;
            self.vol_target = if tune.is_some() { vol } else { 0.0 };
            return;
        }
        if self.tune.is_some() {
            self.pending = Some((tune, vol));
            self.vol_target = 0.0;
            return;
        }
        self.start(tune, vol);
    }

    fn start(&mut self, tune: Option<Tune>, vol: f32) {
        self.tune = tune;
        self.step = 0;
        self.clock = 0.0;
        self.voices.clear();
        self.echo.iter_mut().for_each(|x| *x = 0.0);
        self.sync_echo();
        self.vol_target = if tune.is_some() { vol } else { 0.0 };
    }

    fn noise(rng: &mut u32) -> f32 {
        *rng ^= *rng << 13;
        *rng ^= *rng >> 17;
        *rng ^= *rng << 5;
        *rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn play(&mut self, kind: Kind, note: u8, steps: f32, vol: f32) {
        if self.voices.len() == MAX_VOICES {
            return;
        }
        let gate = steps * 60.0 / self.bpm() / 4.0;
        let freq = if note == 0 { 0.0 } else { midi(note) };
        self.voices.push(Voice { kind, freq, t: 0.0, gate, vol, ph: [0.0, 0.33, 0.66], lp: [0.0; 2] });
    }

    fn trigger_step(&mut self) {
        let bar_abs = self.step / STEPS_PER_BAR;
        let st = self.step % STEPS_PER_BAR;
        let intro = bar_abs < INTRO_BARS;
        let bar = if intro { bar_abs } else { (bar_abs - INTRO_BARS) % LOOP_BARS };
        match self.tune {
            Some(Tune::Shirayuki) => self.city_step(bar, st, intro),
            Some(Tune::TokyoMidnight) => self.midnight_step(bar, st, intro),
            _ => self.synth_step(bar, st, intro),
        }
        self.step = (self.step + 1) % ((INTRO_BARS + LOOP_BARS) * STEPS_PER_BAR);
        if self.step == 0 {
            self.step = INTRO_BARS * STEPS_PER_BAR;
        }
    }

    /// City pop: syncopated electric piano comping, a busy fingered bass,
    /// glockenspiel sparkles in threes against the beat, and a lead hook.
    fn city_step(&mut self, bar: usize, st: usize, intro: bool) {
        let (root, chord) = CITY_CHORDS[bar % 8];
        let with_lead = !intro && bar >= 8;

        if st == 0 {
            for n in chord {
                self.play(Kind::Pad, n, 15.5, 0.022);
            }
        }
        // Push the comping: on the one, the "and" of two, and an anticipation.
        if let Some(len) = match st {
            0 => Some(2.5),
            6 => Some(1.5),
            10 => Some(3.5),
            _ => None,
        } {
            for (k, n) in chord.into_iter().enumerate() {
                self.play(Kind::EPiano, n, len, if k == 3 { 0.05 } else { 0.065 });
            }
        }
        if !intro {
            if st == 0 || st == 7 || st == 10 {
                self.play(Kind::Kick, 0, 1.0, if st == 0 { 0.7 } else { 0.55 });
                self.since_kick = 0.0;
            }
            if st == 4 || st == 12 {
                self.play(Kind::Snare, 0, 1.0, 0.22);
            }
            let hat = match st % 4 {
                2 => 0.07,
                0 => 0.045,
                _ => 0.022,
            };
            self.play(Kind::Hat, 0, 1.0, hat);
            for &(at, interval, len) in &CITY_BASS {
                if at == st {
                    self.play(Kind::Bass, root + interval, len, 0.27);
                }
            }
        }
        // Bells every third step: a gentle cross-rhythm, high above the chord.
        if st.is_multiple_of(3) && st < 15 {
            let pick = (bar * 5 + st / 3) % 4;
            let vol = if with_lead { 0.028 } else { 0.045 };
            self.play(Kind::Bell, chord[pick] + 24, 1.0, vol);
        }
        if with_lead {
            let lb = (bar - 8) as u8;
            for &(phrase, position, note, length) in CITY_MELODY {
                if phrase == lb && position as usize == st {
                    self.play(Kind::Lead, note, length as f32 - 0.3, 0.085);
                }
            }
        }
    }

    fn synth_step(&mut self, bar: usize, st: usize, intro: bool) {
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
            for &(phrase, position, note, length) in MELODY {
                if phrase == lb && position as usize == st {
                    self.play(Kind::Lead, note, length as f32 - 0.3, 0.11);
                }
            }
        }
    }

    fn midnight_step(&mut self, bar: usize, step: usize, intro: bool) {
        let (root, chord) = MIDNIGHT_CHORDS[bar % MIDNIGHT_CHORDS.len()];
        let with_lead = !intro && bar >= 8;
        if step == 0 {
            for note in chord {
                self.play(Kind::Pad, note, 15.5, 0.034);
            }
            if intro {
                self.play(Kind::Bass, root, 6.0, 0.15);
            }
        }
        if !intro {
            if step.is_multiple_of(4) || (bar % 4 == 3 && step == 14) {
                self.play(Kind::Kick, 0, 1.0, if step == 14 { 0.4 } else { 0.66 });
                self.since_kick = 0.0;
            }
            if step == 4 || step == 12 {
                self.play(Kind::Snare, 0, 1.0, 0.24);
            } else if bar % 4 == 3 && step == 15 {
                self.play(Kind::Snare, 0, 1.0, 0.08);
            }
            if step.is_multiple_of(2) || (bar % 4 == 3 && step >= 13) {
                self.play(Kind::Hat, 0, 1.0, if step % 4 == 2 { 0.066 } else { 0.027 });
            }
            for (position, interval, length) in [(0, 0, 2.4), (3, 12, 0.8), (6, 0, 1.4), (8, 0, 2.4), (11, 7, 0.8), (14, 12, 1.4)] {
                if step == position {
                    self.play(Kind::Bass, root + interval, length, 0.28);
                }
            }
        }
        if step.is_multiple_of(2) {
            let pattern = [0, 2, 1, 3, 2, 1, 3, 2];
            let note = chord[pattern[step / 2]] + 12;
            self.play(Kind::Pluck, note, 1.15, if with_lead { 0.052 } else { 0.075 });
        }
        if step == 6 || step == 14 {
            self.play(Kind::Bell, chord[(bar + step / 2) % 4] + 24, 1.0, 0.033);
        }
        if with_lead {
            for &(phrase, position, note, duration) in MIDNIGHT_MELODY {
                if phrase as usize == bar - 8 && position as usize == step {
                    self.play(Kind::Lead, note, duration as f32 - 0.4, 0.078);
                }
            }
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
            if let Some((tune, volume)) = self.pending.take() {
                self.start(tune, volume);
            }
            if self.tune.is_none() {
                return 0.0;
            }
        }
        let step_len = 60.0 / self.bpm() / 4.0 * self.sr;
        let city = self.tune == Some(Tune::Shirayuki);
        if self.clock <= 0.0 {
            self.trigger_step();
            self.clock += step_len;
        }
        self.clock -= 1.0;

        let dt = 1.0 / self.sr;
        self.since_kick += dt;
        let pump = 1.0 - if city { 0.2 } else { 0.55 } * (-self.since_kick * 9.0).exp();
        let (mut dry, mut send) = (0.0f32, 0.0f32);
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
                Kind::EPiano => {
                    // Two-operator FM: a bright bell-like strike mellowing out.
                    v.ph[0] = (v.ph[0] + v.freq * dt).fract();
                    v.ph[1] = (v.ph[1] + v.freq * dt).fract();
                    let index = 1.6 * (-v.t * 7.0).exp() + 0.35;
                    let m = (v.ph[1] * TAU).sin() * index;
                    let trem = 1.0 + 0.12 * (v.t * TAU * 4.2).sin();
                    let o = (v.ph[0] * TAU + m).sin() * env * v.vol * trem * pump;
                    send += o * 0.25;
                    o
                }
                Kind::Bell => {
                    v.ph[0] = (v.ph[0] + v.freq * dt).fract();
                    v.ph[1] = (v.ph[1] + v.freq * 2.76 * dt).fract();
                    let strike = (v.t * 600.0).min(1.0);
                    let tone = (v.ph[0] * TAU).sin() + 0.3 * (v.ph[1] * TAU).sin() * (-v.t * 9.0).exp();
                    let o = tone * (-v.t * 3.2).exp() * strike * v.vol;
                    send += o;
                    o
                }
                Kind::Pluck => {
                    v.ph[0] = (v.ph[0] + v.freq * dt).fract();
                    v.ph[1] = (v.ph[1] + v.freq * 2.005 * dt).fract();
                    let fundamental = (v.ph[0] * TAU).sin();
                    let overtone = (v.ph[1] * TAU).sin() * 0.42 * (-v.t * 12.0).exp();
                    let output = (fundamental + overtone) * (-v.t * 3.5).exp() * env * v.vol;
                    send += output * 0.85;
                    output * pump
                }
                Kind::Kick => {
                    let f = 48.0 + 130.0 * (-v.t * 32.0).exp();
                    v.ph[0] = (v.ph[0] + f * dt).fract();
                    (v.ph[0] * TAU).sin() * (-v.t * 7.0).exp() * v.vol
                }
                Kind::Snare => {
                    let n = Self::noise(&mut self.rng);
                    v.ph[0] = (v.ph[0] + 185.0 * dt).fract();
                    let body = (v.ph[0] * TAU).sin() * (-v.t * 25.0).exp() * 0.5;
                    // Big gated-reverb tail, cut off sharply.
                    let tail = if v.t < 0.28 { 0.35 * (-v.t * 4.0).exp() } else { 0.0 };
                    let o = (n * ((-v.t * 18.0).exp() * 0.7 + tail) + body) * v.vol;
                    send += o * 0.3;
                    o
                }
                Kind::Hat => {
                    let n = Self::noise(&mut self.rng);
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
        self.echo_pos = (self.echo_pos + 1) % self.echo_len;
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
    fn tunes_render_cleanly() {
        render_tune(Tune::NeonSakura, "neon_sakura.wav");
        render_tune(Tune::Shirayuki, "shirayuki.wav");
        render_tune(Tune::TokyoMidnight, "tokyo_midnight.wav");
    }

    fn render_tune(tune: Tune, file: &str) {
        let sr = 44_100.0;
        let mut m = Music::new(sr);
        m.set(Some(tune), 1.0);
        let secs = 50.0;
        let n = (sr * secs) as usize;
        let t = std::time::Instant::now();
        let samples: Vec<f32> = (0..n).map(|_| m.next_sample()).collect();
        let cpu = t.elapsed().as_secs_f32() / secs;
        let peak = samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        let tail = &samples[n / 2..];
        let rms = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
        println!("{tune:?}: peak {peak:.3} rms {rms:.3} cpu {:.2}% voices {}", cpu * 100.0, m.voices.len());
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
        std::fs::write(dir.join(file), wav).unwrap();
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

    #[test]
    fn a_single_track_change_finishes_the_fade_without_frame_updates() {
        let mut music = Music::new(8_000.0);
        music.set(Some(Tune::NeonSakura), 1.0);
        for _ in 0..8_000 { music.next_sample(); }
        music.set(Some(Tune::TokyoMidnight), 1.0);
        for _ in 0..80_000 { assert!(music.next_sample().is_finite()); }
        assert_eq!(music.tune, Some(Tune::TokyoMidnight));
        assert!(music.pending.is_none());
        assert!(music.vol > 0.99);
        music.set(Some(Tune::Shirayuki), 0.45);
        music.set(Some(Tune::TokyoMidnight), 0.45);
        assert!(music.pending.is_none());
        for _ in 0..16_000 { music.next_sample(); }
        assert_eq!(music.tune, Some(Tune::TokyoMidnight));
        assert!((music.vol - 0.45).abs() < 0.001);
    }
}
