import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createOutput } from "../web/audio.mjs";

const module = new WebAssembly.Module(await readFile(new URL("../web/pkg/slopracer_f1_bg.wasm", import.meta.url)));
const nativeEncoder = new TextEncoder();
globalThis.TextEncoder = undefined;
globalThis.TextDecoder = undefined;
let Processor;
globalThis.sampleRate = 48_000;
globalThis.AudioWorkletProcessor = class {
    constructor() { this.port = { onmessage: null }; }
};
globalThis.registerProcessor = (name, processor) => {
    assert.equal(name, "slopracer-audio");
    Processor = processor;
};
await import("../web/audio-worklet.mjs");

test("worklet UTF-8 support handles Japanese labels and multibyte characters", () => {
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    for (const text of ["", "Slopracer", "ネオン 東京", "Night 🌙", "\ud800"]) {
        assert.deepEqual(encoder.encode(text), nativeEncoder.encode(text));
        assert.equal(decoder.decode(encoder.encode(text)), text === "\ud800" ? "\ufffd" : text);
    }
    const small = new Uint8Array(3);
    assert.deepEqual(encoder.encodeInto("🌙", small), { read: 0, written: 0 });
});

globalThis.document = { baseURI: "https://example.com/game/" };
globalThis.AudioContext = class {
    constructor() {
        this.state = "suspended";
        this.destination = {};
        this.audioWorklet = { addModule: async url => assert.equal(url.href, "https://example.com/game/audio-worklet.mjs") };
    }
    async resume() { this.state = "running"; }
    async close() { this.state = "closed"; }
};
globalThis.AudioWorkletNode = class {
    constructor(_context, _name, options) {
        this.processor = new Processor(options);
        this.messages = [];
        this.port = {
            postMessage: data => {
                this.messages.push(data);
                this.processor.port.onmessage({ data });
            },
            close() {},
        };
    }
    connect() {}
    disconnect() {}
};

test("browser output preserves startup controls and does not flood the audio thread", async () => {
    const output = createOutput(module, false);
    output.engine(0.7, true, true);
    output.music(3, 1);
    output.effect(1);
    await output.ready;
    assert.equal(output.available(), true);
    assert.equal(output.node.messages.length, 4);
    for (let frame = 0; frame < 300; frame++) {
        output.music(3, 1);
        output.engine(0.7, true, true);
    }
    assert.equal(output.node.messages.length, 4);
    output.close();
    assert.equal(output.available(), false);
});

test("the actual WASM processor renders continuously without game-frame updates", () => {
    for (const tune of [1, 2, 3]) {
        const processor = new Processor({ processorOptions: { module, muted: false } });
        processor.port.onmessage({ data: { kind: "music", tune, volume: 1 } });
        const left = new Float32Array(128);
        const right = new Float32Array(128);
        const outputs = [[left, right]];
        const buffer = processor.samples.buffer;
        for (let block = 0; block < 2_000; block++) {
            assert.equal(processor.process([], outputs), true);
            assert.deepEqual(left, right);
            assert.ok(left.every(Number.isFinite));
            if (block > 100) assert.ok(left.some(sample => Math.abs(sample) > 0.001));
        }
        assert.equal(processor.samples.buffer, buffer);
        processor.port.onmessage({ data: { kind: "close" } });
        assert.equal(processor.process([], outputs), false);
    }
});

test("variable audio block sizes, silence, and resume preserve the output contract", () => {
    const processor = new Processor({ processorOptions: { module, muted: false } });
    processor.port.onmessage({ data: { kind: "music", tune: 3, volume: 1 } });
    for (const frames of [128, 256, 512, 4_096]) {
        const output = [new Float32Array(frames), new Float32Array(frames)];
        processor.process([], [output]);
        assert.ok(output[0].some(sample => sample !== 0));
        assert.deepEqual(output[0], output[1]);
    }
    processor.port.onmessage({ data: { kind: "mute", muted: true } });
    const silent = new Float32Array(128).fill(1);
    processor.process([], [[silent]]);
    assert.ok(silent.every(sample => sample === 0));
    processor.port.onmessage({ data: { kind: "mute", muted: false } });
    processor.process([], [[silent]]);
    assert.ok(silent.some(sample => sample !== 0));
    processor.port.onmessage({ data: { kind: "close" } });
});
