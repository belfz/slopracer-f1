import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

const playwrightPath = process.env.PLAYWRIGHT_MODULE;
const { chromium } = await import(playwrightPath ? pathToFileURL(playwrightPath).href : "playwright");

test("browser audio survives rendering-thread stalls and game startup", { timeout: 60_000 }, async () => {
    const root = fileURLToPath(new URL("../web/", import.meta.url));
    const server = createServer(async (request, response) => {
        try {
            const pathname = new URL(request.url, "http://localhost").pathname;
            const filename = path.join(root, pathname === "/" ? "index.html" : pathname);
            const bytes = await readFile(filename);
            const contentTypes = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".png": "image/png" };
            response.setHeader("Content-Type", contentTypes[path.extname(filename)] || "application/octet-stream");
            response.end(bytes);
        } catch {
            response.writeHead(404);
            response.end();
        }
    });
    await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
    let browser;
    try {
        browser = await chromium.launch({
            executablePath: process.env.CHROME_EXECUTABLE || undefined,
            headless: true,
            args: ["--mute-audio", "--autoplay-policy=no-user-gesture-required"],
        });
        const page = await browser.newPage();
        const errors = [];
        page.on("pageerror", error => errors.push(String(error)));
        page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
        await page.goto(`http://127.0.0.1:${server.address().port}/`);
        const result = await page.evaluate(async () => {
            const { createOutput } = await import("./audio.mjs");
            const bytes = await (await fetch("./pkg/slopracer_f1_bg.wasm")).arrayBuffer();
            const output = createOutput(await WebAssembly.compile(bytes), false);
            output.music(3, 1);
            output.engine(0.7, true, true);
            await output.ready;
            if (!output.available()) return { available: false };
            const source = `class Monitor extends AudioWorkletProcessor {
                constructor() {
                    super();
                    this.blocks = 0;
                    this.zeroBlocks = 0;
                    this.started = false;
                    this.port.onmessage = () => this.port.postMessage({blocks:this.blocks, zeroBlocks:this.zeroBlocks});
                }
                process(inputs, outputs) {
                    const input = inputs[0]?.[0];
                    if (input) {
                        let energy = 0;
                        for (const sample of input) energy += sample * sample;
                        if (energy > 0.00001) this.started = true;
                        if (this.started) {
                            this.blocks++;
                            if (energy === 0) this.zeroBlocks++;
                        }
                        for (const channel of outputs[0]) channel.set(input);
                    }
                    return true;
                }
            } registerProcessor("audio-monitor", Monitor);`;
            const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
            await output.context.audioWorklet.addModule(url);
            URL.revokeObjectURL(url);
            const monitor = new AudioWorkletNode(output.context, "audio-monitor");
            output.node.disconnect();
            output.node.connect(monitor);
            monitor.connect(output.context.destination);
            await new Promise(resolve => setTimeout(resolve, 5_000));
            for (let freeze = 0; freeze < 4; freeze++) {
                const until = performance.now() + 250;
                while (performance.now() < until) Math.sqrt(Math.random());
                await new Promise(resolve => setTimeout(resolve, 20));
            }
            const stats = await new Promise((resolve, reject) => {
                const timeout = setTimeout(() => reject(new Error("Audio monitor did not respond")), 5_000);
                monitor.port.onmessage = event => { clearTimeout(timeout); resolve(event.data); };
                monitor.port.postMessage("report");
            });
            const available = output.available();
            output.close();
            return { ...stats, available };
        });
        assert.deepEqual(errors, []);
        assert.equal(result.available, true);
        assert.ok(result.blocks > 200, `too few audio blocks: ${result.blocks}`);
        assert.equal(result.zeroBlocks, 0);
        console.log("Audio stress result:", result);

        await page.addInitScript(() => {
            window.audioTest = { nodes: [], messages: [], errors: [], frames: [] };
            const OriginalNode = window.AudioWorkletNode;
            window.AudioWorkletNode = class extends OriginalNode {
                constructor(...args) {
                    super(...args);
                    window.audioTest.nodes.push(this);
                    this.addEventListener("processorerror", () => window.audioTest.errors.push("processorerror"));
                    const send = this.port.postMessage.bind(this.port);
                    this.port.postMessage = message => { window.audioTest.messages.push(message); send(message); };
                }
            };
            const requestFrame = window.requestAnimationFrame.bind(window);
            window.requestAnimationFrame = callback => requestFrame(timestamp => {
                const start = performance.now();
                callback(timestamp);
                window.audioTest.frames.push(performance.now() - start);
            });
        });
        await page.reload();
        await page.waitForFunction(() => document.querySelector("#status")?.textContent.includes("Press any key"));
        await page.locator("#power").click();
        await page.waitForFunction(() => window.audioTest.nodes.length === 1);
        await page.keyboard.press("Enter");
        await page.waitForFunction(() => window.audioTest.messages.some(message => message.kind === "music" && message.tune === 1));
        await page.keyboard.press("ArrowDown");
        const timings = [];
        for (const tune of [1, 2, 3]) {
            if (tune !== 1) await page.keyboard.press("ArrowRight");
            await page.waitForFunction(tune => window.audioTest.messages.some(message => message.kind === "music" && message.tune === tune), tune);
            await page.evaluate(() => { window.audioTest.frames.length = 0; });
            await page.waitForFunction(() => window.audioTest.frames.length >= 30);
            timings.push(await page.evaluate(tune => {
                const frames = window.audioTest.frames;
                return { tune, averageMs: frames.reduce((sum, value) => sum + value, 0) / frames.length, maxMs: Math.max(...frames) };
            }, tune));
        }
        await page.keyboard.press("Enter");
        await page.waitForFunction(() => window.audioTest.messages.some(message => message.kind === "music" && message.tune === 3 && message.volume === 1));
        await page.keyboard.press("m");
        await page.waitForFunction(() => window.audioTest.messages.some(message => message.kind === "mute" && message.muted));
        await page.keyboard.press("m");
        await page.waitForFunction(() => window.audioTest.messages.filter(message => message.kind === "mute" && !message.muted).length >= 2);
        assert.deepEqual(await page.evaluate(() => window.audioTest.errors), []);
        assert.deepEqual(errors, []);
        console.log("Game rendering comparison:", timings);
    } finally {
        if (browser) await browser.close();
        await new Promise(resolve => server.close(resolve));
    }
});
