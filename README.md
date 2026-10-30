<img src="assets/icon.svg" width="64" height="64" alt="">

# pegasus-f1

A retro pseudo-3D Formula racer for the terminal, inspired by the F1
cartridges on the Pegasus / Famicom-clone consoles, with graphics a console
generation later: 16-bit style. Written in Rust with [ratatui](https://ratatui.rs).
Sound comes from a tiny chiptune-style synth (pulse-wave engine, beeps and noise)
via [cpal](https://github.com/RustAudio/cpal), plus a synthwave soundtrack
for the Neon Sakura track.

![Danube Ring](assets/screenshot-day.png)
![Carpathian Circuit](assets/screenshot-sunset.png)
![Baltic Night GP](assets/screenshot-night.png)
![Neon Sakura](assets/screenshot-synthwave.png)

## Play

```sh
cargo run --release
```

Every start plays a short "Crystal BIOS" self-test that types out your real
terminal setup before the menu. Press any key to skip it, or start with
`--no-intro`.

Options: `--mute`, `--no-intro`, `--truecolor` / `--256` (colour mode is auto-detected),
`--pixels` / `--quadrant` / `--halfblock` (graphics mode, see below).

## Graphics modes

Switch between modes at any time with `V` (or in the menu):

| Mode | Resolution | Works in |
| --- | --- | --- |
| **Pixels** | true ~16-bit console resolution (e.g. 384×216) | kitty, Ghostty, WezTerm (kitty graphics protocol, auto-detected) |
| **Quadrant** (default elsewhere) | 2×2 pixels per character cell | any terminal with a Unicode font |
| **Half blocks** | 1×2 pixels per character cell | any terminal |

In the text modes, the resolution depends on the terminal size, so a smaller
font (`Cmd -`) or a bigger window gives sharper graphics.

| Key | Action |
| --- | --- |
| `←` `→` / `A` `D` | Steer |
| `↑` / `W` | Accelerate |
| `↓` / `S` | Brake |
| `Space` / `G` | Shift gear LO ⇄ HI (manual gearbox) |
| `P` / `Esc` | Pause |
| `M` | Mute |
| `V` | Cycle graphics mode |
| `Q` | Quit (from menu / pause) |

Start from the back of a 12-car grid, wait for the five red lights to go
out, and fight your way to the front over 3 laps. Watch for crests: climbing
costs speed, and you can't see what's behind the top of a hill. Shift up to HI at around
100 km/h: LO gear tops out near 200 km/h. Bends throw you outward, so ease
off in the tight ones. Going off the road slows you down, and hitting a tree,
a sign or another car sends you spinning.

**Tip:** for the best controls, use a terminal that supports the kitty keyboard
protocol (kitty, WezTerm, Ghostty, foot, recent iTerm2/Alacritty). It reports
key releases, so you hold `↑` to accelerate. In other terminals (e.g. macOS
Terminal.app) the game switches to *auto-throttle*: the car accelerates on its
own and you hold `↓` to brake.

## Tracks

- **Danube Ring**: sunny parkland, fast flowing bends
- **Carpathian Circuit**: twisty mountain road at sunset
- **Baltic Night GP**: long straights and hairpins under the lights
- **Neon Sakura**: synthwave sunset with a striped retro sun, a neon grid
  floor, palm-lined boulevards, katakana neon signs and a tunnel of torii
  gates. It has its own soundtrack (previewed softly in the menu when the
  track is selected).
