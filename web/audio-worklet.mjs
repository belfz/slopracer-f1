import "./audio-codecs.mjs";
import { AudioSynth, initSync } from "./pkg/slopracer_f1.js";

class SlopracerAudio extends AudioWorkletProcessor {
    constructor(options) {
        super();
        this.memory = initSync({ module: options.processorOptions.module }).memory;
        this.synth = new AudioSynth(sampleRate, options.processorOptions.muted);
        this.pointer = this.synth.samples_ptr();
        this.capacity = this.synth.capacity();
        this.samples = new Float32Array(this.memory.buffer, this.pointer, this.capacity);
        this.closed = false;
        this.port.onmessage = ({ data }) => {
            if (this.closed) return;
            switch (data.kind) {
                case "engine": this.synth.engine(data.rpm, data.throttle, data.on); break;
                case "music": this.synth.music(data.tune, data.volume); break;
                case "effect": this.synth.effect(data.effect); break;
                case "mute": this.synth.mute(data.muted); break;
                case "close": this.synth.free(); this.closed = true; break;
            }
        };
    }

    process(_inputs, outputs) {
        if (this.closed) return false;
        const channels = outputs[0];
        if (!channels?.length) return true;
        const frames = channels[0].length;
        for (let offset = 0; offset < frames; offset += this.capacity) {
            const count = Math.min(this.capacity, frames - offset);
            this.synth.render(count);
            if (this.samples.buffer !== this.memory.buffer) {
                this.samples = new Float32Array(this.memory.buffer, this.pointer, this.capacity);
            }
            for (const channel of channels) {
                for (let index = 0; index < count; index++) channel[offset + index] = this.samples[index];
            }
        }
        return true;
    }
}

registerProcessor("slopracer-audio", SlopracerAudio);
