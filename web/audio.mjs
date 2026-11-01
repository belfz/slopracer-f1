export function createOutput(module, muted) {
    return new Output(module, muted);
}

class Output {
    constructor(module, muted) {
        this.context = new AudioContext({ latencyHint: "interactive" });
        this.node = null;
        this.failed = false;
        this.closed = false;
        this.engineState = [0, false, false];
        this.musicState = [0, 0];
        this.muted = muted;
        this.pendingEffects = [];
        this.context.resume().catch(error => this.fail(error));
        this.ready = this.start(module).catch(error => this.fail(error));
    }

    async start(module) {
        const processorUrl = new URL("./audio-worklet.mjs", document.baseURI);
        await this.context.audioWorklet.addModule(processorUrl);
        if (this.closed) return;
        this.node = new AudioWorkletNode(this.context, "slopracer-audio", {
            numberOfInputs: 0,
            numberOfOutputs: 1,
            outputChannelCount: [2],
            processorOptions: { module, muted: this.muted },
        });
        this.node.onprocessorerror = () => this.fail(new Error("Audio processor failed"));
        this.send({ kind: "engine", rpm: this.engineState[0], throttle: this.engineState[1], on: this.engineState[2] });
        this.send({ kind: "music", tune: this.musicState[0], volume: this.musicState[1] });
        this.send({ kind: "mute", muted: this.muted });
        for (const effect of this.pendingEffects) this.send({ kind: "effect", effect });
        this.pendingEffects.length = 0;
        this.node.connect(this.context.destination);
    }

    send(message) {
        this.node?.port.postMessage(message);
    }

    available() {
        return !this.failed && !this.closed;
    }

    engine(rpm, throttle, on) {
        if (this.engineState[0] === rpm && this.engineState[1] === throttle && this.engineState[2] === on) return;
        this.engineState = [rpm, throttle, on];
        this.send({ kind: "engine", rpm, throttle, on });
    }

    music(tune, volume) {
        if (this.musicState[0] === tune && this.musicState[1] === volume) return;
        this.musicState = [tune, volume];
        this.send({ kind: "music", tune, volume });
    }

    effect(effect) {
        if (this.node) this.send({ kind: "effect", effect });
        else if (this.pendingEffects.length < 32) this.pendingEffects.push(effect);
    }

    mute(muted) {
        this.muted = muted;
        this.send({ kind: "mute", muted });
    }

    fail(error) {
        this.failed = true;
        console.error("Slopracer audio:", error);
        this.close();
    }

    close() {
        this.closed = true;
        if (this.node) {
            this.send({ kind: "close" });
            this.node.disconnect();
            this.node.port.close();
        }
        if (this.context.state !== "closed") this.context.close().catch(() => {});
    }
}
