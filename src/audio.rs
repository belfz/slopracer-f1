//! A tiny 8-bit style synthesizer: a pulse-wave engine drone plus one-shot
//! square/triangle/noise voices for effects. Silently disabled if no audio
//! device is available.

#[cfg(not(target_arch = "wasm32"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crate::music::{Music, Tune};
#[cfg(not(target_arch = "wasm32"))]
use cpal::{FromSample, SizedSample};
use std::cell::Cell;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MAX_EFFECT_VOICES: usize = 32;
const AUDIO_BLOCK: usize = 2048;

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
    fn new(sr: f32, muted: bool) -> Self {
        Self {
            sr,
            engine_f: 50.0,
            engine_target: 50.0,
            engine_vol: 0.0,
            engine_vol_target: 0.0,
            ph1: 0.0,
            ph2: 0.0,
            voices: Vec::with_capacity(MAX_EFFECT_VOICES),
            lfsr: 1,
            lp: 0.0,
            muted,
            music: Music::new(sr),
        }
    }

    fn set_engine(&mut self, rpm: f32, throttle: bool, on: bool) {
        self.engine_target = 45.0 + rpm.max(0.0) * 200.0;
        self.engine_vol_target = if on { if throttle { 0.10 } else { 0.06 } } else { 0.0 };
    }

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
        if self.voices.len() == MAX_EFFECT_VOICES {
            return;
        }
        self.voices.push(Voice { wave, f0, f1, dur, delay, vol, t: 0.0, phase: 0.0, noise: 1.0 });
    }
}

pub struct Audio {
    muted: Cell<bool>,
    #[cfg(not(target_arch = "wasm32"))]
    commands: SyncSender<Command>,
    #[cfg(not(target_arch = "wasm32"))]
    output_muted: Arc<AtomicBool>,
    #[cfg(not(target_arch = "wasm32"))]
    _stream: Option<cpal::Stream>,
    #[cfg(target_arch = "wasm32")]
    output: Option<WebOutput>,
}

#[cfg(not(target_arch = "wasm32"))]
enum Command {
    Engine(f32, bool, bool),
    Music(Option<Tune>, f32),
    Effect(Sfx),
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (commands, receiver) = sync_channel(128);
            let output_muted = Arc::new(AtomicBool::new(muted));
            let stream = open_stream(receiver, output_muted.clone());
            Audio { muted: Cell::new(muted), commands, output_muted, _stream: stream }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let output = create_output(&wasm_bindgen::module(), muted).ok();
            Audio { muted: Cell::new(muted), output }
        }
    }

    pub fn available(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        { self._stream.is_some() }
        #[cfg(target_arch = "wasm32")]
        { self.output.as_ref().is_some_and(WebOutput::available) }
    }

    pub fn toggle_mute(&self) -> bool {
        let muted = !self.muted.get();
        self.muted.set(muted);
        #[cfg(not(target_arch = "wasm32"))]
        self.output_muted.store(muted, Ordering::Relaxed);
        #[cfg(target_arch = "wasm32")]
        if let Some(output) = &self.output { output.mute(muted); }
        muted
    }

    /// `rpm` in 0..~1.1; `on` false silences the engine.
    pub fn set_engine(&self, rpm: f32, throttle: bool, on: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.commands.try_send(Command::Engine(rpm, throttle, on));
        #[cfg(target_arch = "wasm32")]
        if let Some(output) = &self.output { output.engine(rpm, throttle, on); }
    }

    /// Play a track's soundtrack (or none) at volume `vol`, with fades.
    pub fn set_music(&self, tune: Option<Tune>, vol: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.commands.try_send(Command::Music(tune, vol));
        #[cfg(target_arch = "wasm32")]
        if let Some(output) = &self.output {
            output.music(match tune { None => 0, Some(Tune::NeonSakura) => 1, Some(Tune::Shirayuki) => 2, Some(Tune::TokyoMidnight) => 3 }, vol);
        }
    }

    pub fn play(&self, sfx: Sfx) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.commands.try_send(Command::Effect(sfx));
        #[cfg(target_arch = "wasm32")]
        if let Some(output) = &self.output { output.effect(sfx as u8); }
    }
}

impl Synth {
    fn play(&mut self, sfx: Sfx) {
        match sfx {
            Sfx::Light => self.add(Wave::Square, 440.0, 440.0, 0.3, 0.0, 0.18),
            Sfx::Go => self.add(Wave::Square, 880.0, 880.0, 0.7, 0.0, 0.2),
            Sfx::Bump => self.add(Wave::Noise, 900.0, 200.0, 0.18, 0.0, 0.35),
            Sfx::Crash => {
                self.add(Wave::Noise, 3000.0, 60.0, 1.1, 0.0, 0.5);
                self.add(Wave::Square, 220.0, 40.0, 0.5, 0.0, 0.15);
            }
            Sfx::Lap => {
                for (i, f) in [523.0, 659.0, 784.0].into_iter().enumerate() {
                    self.add(Wave::Square, f, f, 0.12, i as f32 * 0.1, 0.15);
                }
            }
            Sfx::Finish => {
                let notes = [(523.0, 0.12), (659.0, 0.12), (784.0, 0.12), (1047.0, 0.3), (784.0, 0.12), (1047.0, 0.6)];
                let mut t = 0.0;
                for (f, d) in notes {
                    self.add(Wave::Square, f, f, d, t, 0.16);
                    self.add(Wave::Triangle, f / 2.0, f / 2.0, d, t, 0.25);
                    t += d + 0.03;
                }
            }
            Sfx::Shift => self.add(Wave::Triangle, 300.0, 600.0, 0.06, 0.0, 0.3),
            Sfx::Blip => self.add(Wave::Square, 660.0, 660.0, 0.04, 0.0, 0.12),
            Sfx::Select => {
                self.add(Wave::Square, 660.0, 660.0, 0.06, 0.0, 0.14);
                self.add(Wave::Square, 990.0, 990.0, 0.12, 0.06, 0.14);
            }
            Sfx::Tick => self.add(Wave::Square, 1800.0, 1800.0, 0.012, 0.0, 0.05),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open_stream(receiver: Receiver<Command>, muted: Arc<AtomicBool>) -> Option<cpal::Stream> {
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    let format = config.sample_format();
    let cfg: cpal::StreamConfig = config.into();
    let synth = Synth::new(cfg.sample_rate.0 as f32, muted.load(Ordering::Relaxed));
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &cfg, synth, receiver, muted),
        cpal::SampleFormat::I16 => build::<i16>(&device, &cfg, synth, receiver, muted),
        cpal::SampleFormat::U16 => build::<u16>(&device, &cfg, synth, receiver, muted),
        _ => None,
    }?;
    stream.play().ok()?;
    Some(stream)
}

#[cfg(not(target_arch = "wasm32"))]
fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    mut synth: Synth,
    receiver: Receiver<Command>,
    muted: Arc<AtomicBool>,
) -> Option<cpal::Stream> {
    let channels = cfg.channels as usize;
    device
        .build_output_stream(
            cfg,
            move |data: &mut [T], _| {
                synth.cleanup();
                for command in receiver.try_iter() {
                    match command {
                        Command::Engine(rpm, throttle, on) => synth.set_engine(rpm, throttle, on),
                        Command::Music(tune, volume) => synth.music.set(tune, volume),
                        Command::Effect(effect) => synth.play(effect),
                    }
                }
                synth.muted = muted.load(Ordering::Relaxed);
                for frame in data.chunks_mut(channels) {
                    let sample = T::from_sample(synth.next_sample());
                    frame.fill(sample);
                }
            },
            |_err| {},
            None,
        )
        .ok()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/web/audio.mjs")]
extern "C" {
    type WebOutput;
    #[wasm_bindgen(catch, js_name = createOutput)]
    fn create_output(module: &JsValue, muted: bool) -> Result<WebOutput, JsValue>;
    #[wasm_bindgen(method)]
    fn available(this: &WebOutput) -> bool;
    #[wasm_bindgen(method)]
    fn engine(this: &WebOutput, rpm: f32, throttle: bool, on: bool);
    #[wasm_bindgen(method)]
    fn music(this: &WebOutput, tune: u8, volume: f32);
    #[wasm_bindgen(method)]
    fn effect(this: &WebOutput, effect: u8);
    #[wasm_bindgen(method)]
    fn mute(this: &WebOutput, muted: bool);
    #[wasm_bindgen(method)]
    fn close(this: &WebOutput);
}

#[cfg(target_arch = "wasm32")]
impl Drop for Audio {
    fn drop(&mut self) {
        if let Some(output) = &self.output { output.close(); }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct AudioSynth {
    synth: Synth,
    samples: Vec<f32>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl AudioSynth {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(sample_rate: f32, muted: bool) -> Self {
        Self { synth: Synth::new(sample_rate, muted), samples: vec![0.0; AUDIO_BLOCK] }
    }

    pub fn samples_ptr(&self) -> *const f32 {
        self.samples.as_ptr()
    }

    pub fn capacity(&self) -> usize {
        self.samples.len()
    }

    pub fn render(&mut self, count: usize) {
        self.synth.cleanup();
        for sample in self.samples.iter_mut().take(count) {
            *sample = self.synth.next_sample();
        }
    }

    pub fn engine(&mut self, rpm: f32, throttle: bool, on: bool) {
        self.synth.set_engine(rpm, throttle, on);
    }

    pub fn music(&mut self, tune: u8, volume: f32) {
        let tune = match tune { 1 => Some(Tune::NeonSakura), 2 => Some(Tune::Shirayuki), 3 => Some(Tune::TokyoMidnight), _ => None };
        self.synth.music.set(tune, volume);
    }

    pub fn effect(&mut self, effect: u8) {
        let effects = [Sfx::Light, Sfx::Go, Sfx::Bump, Sfx::Crash, Sfx::Lap, Sfx::Finish, Sfx::Shift, Sfx::Blip, Sfx::Select, Sfx::Tick];
        if let Some(&effect) = effects.get(effect as usize) { self.synth.play(effect); }
    }

    pub fn mute(&mut self, muted: bool) {
        self.synth.muted = muted;
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};

    #[test]
    fn native_mute_does_not_depend_on_command_queue_capacity() {
        let (commands, receiver) = sync_channel(1);
        assert!(commands.try_send(Command::Effect(Sfx::Go)).is_ok());
        let output_muted = Arc::new(AtomicBool::new(false));
        let audio = Audio { muted: Cell::new(false), commands, output_muted: output_muted.clone(), _stream: None };
        for expected in [true, false, true, false] {
            assert_eq!(audio.toggle_mute(), expected);
            assert_eq!(output_muted.load(Ordering::Relaxed), expected);
            assert!(matches!(audio.commands.try_send(Command::Effect(Sfx::Tick)), Err(std::sync::mpsc::TrySendError::Full(_))));
        }
        assert!(matches!(receiver.try_recv(), Ok(Command::Effect(Sfx::Go))));
    }

    struct TrackedAllocator;

    thread_local! {
        static HEAP_OPERATIONS: Cell<Option<usize>> = const { Cell::new(None) };
    }

    fn track_heap_operation() {
        let _ = HEAP_OPERATIONS.try_with(|counter| {
            if let Some(count) = counter.get() { counter.set(Some(count + 1)); }
        });
    }

    unsafe impl GlobalAlloc for TrackedAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            track_heap_operation();
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            track_heap_operation();
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            track_heap_operation();
            unsafe { System.realloc(pointer, layout, size) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            track_heap_operation();
            unsafe { System.dealloc(pointer, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: TrackedAllocator = TrackedAllocator;

    #[test]
    fn playback_does_not_allocate_or_free_memory() {
        let mut output = AudioSynth::new(8_000.0, false);
        HEAP_OPERATIONS.with(|counter| counter.set(Some(0)));
        for tune in 1..=3 {
            output.music(tune, 1.0);
            for block in 0..3_200 {
                if block % 400 == 0 { output.effect(Sfx::Finish as u8); }
                output.engine(0.7, true, true);
                output.render(128);
            }
        }
        let operations = HEAP_OPERATIONS.with(|counter| counter.replace(None).unwrap());
        assert_eq!(operations, 0, "audio rendering must not use the heap");
    }

    #[test]
    fn block_boundaries_do_not_interrupt_or_repeat_audio() {
        for rate in [44_100.0, 48_000.0] {
            for (index, tune) in [Tune::NeonSakura, Tune::Shirayuki, Tune::TokyoMidnight].into_iter().enumerate() {
                let mut output = AudioSynth::new(rate, false);
                let mut reference = Synth::new(rate, false);
                output.music(index as u8 + 1, 1.0);
                reference.music.set(Some(tune), 1.0);
                for count in [1, 128, 256, 2048, 128, 1] {
                    output.render(count);
                    for &sample in &output.samples[..count] {
                        assert_eq!(sample, reference.next_sample());
                        assert!(sample.is_finite());
                    }
                }
            }
        }
    }

    #[test]
    fn effect_bursts_are_bounded_and_muting_keeps_time_moving() {
        let mut output = AudioSynth::new(48_000.0, false);
        output.music(3, 1.0);
        for _ in 0..1_000 { output.effect(Sfx::Finish as u8); }
        assert_eq!(output.synth.voices.len(), MAX_EFFECT_VOICES);
        output.mute(true);
        output.render(128);
        assert!(output.samples[..128].iter().all(|&sample| sample == 0.0));
        assert!(output.synth.music.next_sample().is_finite());
        output.mute(false);
        output.render(128);
        assert!(output.samples[..128].iter().any(|sample| sample.abs() > 0.001));
    }
}
