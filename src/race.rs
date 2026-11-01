//! Race simulation: player physics, rival AI, collisions and lap timing.

use crate::audio::Sfx;
use crate::gfx::Rgb;
use crate::render::{SpriteInst, PLAYER_Z};
use crate::sprites;
use crate::track::{Track, ROAD_HALF};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::rc::Rc;

const LANES: [f32; 3] = [-3.3, 0.0, 3.3];
const STEER_SPEED: f32 = 9.5;
const CENTRIFUGAL: f32 = 0.1;
const CAR_LEN: f32 = 4.3;
const CAR_HALF_W: f32 = 0.95;
pub const GRID_SIZE: usize = 12;

const TEAM_COLORS: [Rgb; GRID_SIZE - 1] = [
    Rgb(30, 80, 230),
    Rgb(250, 200, 20),
    Rgb(20, 170, 80),
    Rgb(255, 120, 0),
    Rgb(0, 200, 210),
    Rgb(140, 60, 200),
    Rgb(235, 235, 235),
    Rgb(255, 90, 170),
    Rgb(150, 150, 160),
    Rgb(10, 90, 50),
    Rgb(20, 30, 110),
];

#[derive(Clone, Copy, PartialEq)]
pub enum Gear {
    Lo,
    Hi,
}

#[derive(Clone, Copy, Default)]
pub struct Controls {
    /// -1 (left) ..= 1 (right)
    pub steer: f32,
    pub accel: bool,
    pub brake: bool,
}

pub struct Rival {
    pub d: f32,
    pub x: f32,
    pub v: f32,
    top: f32,
    target_x: f32,
    lane_timer: f32,
    color: Rgb,
}

pub enum Phase {
    Countdown { t: f32, lights: u32, go_at: f32 },
    Racing,
    Finished { t: f32 },
}

pub struct Race {
    pub track: Rc<Track>,
    pub laps: u32,
    pub demo: bool,
    pub auto_gear: bool,
    pub car_color: Rgb,

    pub d: f32,
    pub x: f32,
    pub v: f32,
    pub gear: Gear,
    pub rev: f32,
    pub throttle: bool,
    pub offroad: bool,
    steer_vis: f32,
    spin: f32,
    /// Where the car is pulled back to after hitting roadside scenery.
    recover_x: Option<f32>,
    /// Seconds of immunity to scenery after a crash, so you can drive away.
    grace: f32,
    ai_x: f32,

    pub rivals: Vec<Rival>,
    pub phase: Phase,
    pub time: f32,
    lap_start: f32,
    pub lap: u32,
    pub last_lap: Option<f32>,
    pub best_lap: Option<f32>,
    pub final_pos: usize,
    pub bg: f32,
    pub sfx: Vec<Sfx>,
    pub message: Option<(String, f32)>,
    rng: StdRng,
}

/// A rival whose team colour is too close to the player's car takes red.
fn rival_color(team: Rgb, player: Rgb) -> Rgb {
    let d = |a: u8, b: u8| (a as i32 - b as i32).abs();
    if d(team.0, player.0) + d(team.1, player.1) + d(team.2, player.2) < 120 {
        let red = Rgb(225, 25, 35);
        if d(red.0, player.0) + d(red.1, player.1) + d(red.2, player.2) < 120 { Rgb(235, 235, 235) } else { red }
    } else {
        team
    }
}

fn grid_slot(k: usize) -> (f32, f32) {
    let d = -5.0 - (k / 2) as f32 * 12.0 - (k % 2) as f32 * 6.0;
    let x = if k.is_multiple_of(2) { -2.2 } else { 2.2 };
    (d, x)
}

fn sweep_interval(start: f32, end: f32, lower: f32, upper: f32) -> Option<(f32, f32)> {
    if lower > upper {
        return None;
    }
    let movement = end - start;
    if movement == 0.0 {
        return (start >= lower && start <= upper).then_some((0.0, 1.0));
    }
    let first = (lower - start) / movement;
    let last = (upper - start) / movement;
    let enter = first.min(last).max(0.0);
    let leave = first.max(last).min(1.0);
    (enter <= leave).then_some((enter, leave))
}

impl Race {
    pub fn new(track: Rc<Track>, laps: u32, auto_gear: bool, demo: bool) -> Race {
        let mut rng = StdRng::from_entropy();
        let rivals = (0..GRID_SIZE - 1)
            .map(|k| {
                let (d, x) = grid_slot(k);
                Rival {
                    d,
                    x,
                    v: 0.0,
                    top: 84.0 - k as f32 * 1.5 + rng.gen_range(-1.0..1.0),
                    target_x: x,
                    lane_timer: rng.gen_range(3.0..8.0),
                    color: rival_color(TEAM_COLORS[k], track.palette.car),
                }
            })
            .collect();
        let (d, x) = grid_slot(GRID_SIZE - 1);
        let phase = if demo {
            Phase::Racing
        } else {
            Phase::Countdown { t: 0.0, lights: 0, go_at: 5.0 + rng.gen_range(0.4..1.5) }
        };
        let car_color = track.palette.car;
        Race {
            track,
            laps,
            demo,
            auto_gear,
            car_color,
            d,
            x,
            v: 0.0,
            gear: Gear::Lo,
            rev: 0.15,
            throttle: false,
            offroad: false,
            steer_vis: 0.0,
            spin: 0.0,
            recover_x: None,
            grace: 0.0,
            ai_x: x,
            rivals,
            phase,
            time: 0.0,
            lap_start: 0.0,
            lap: 1,
            last_lap: None,
            best_lap: None,
            final_pos: 0,
            bg: 0.0,
            sfx: Vec::new(),
            message: None,
            rng,
        }
    }

    pub fn position(&self) -> usize {
        1 + self.rivals.iter().filter(|r| r.d > self.d).count()
    }

    pub fn lights(&self) -> Option<u32> {
        match self.phase {
            Phase::Countdown { lights, .. } => Some(lights),
            _ => None,
        }
    }

    pub fn finished_for(&self) -> Option<f32> {
        match self.phase {
            Phase::Finished { t } => Some(t),
            _ => None,
        }
    }

    pub fn shift(&mut self) {
        if !self.auto_gear && self.spin <= 0.0 {
            self.gear = if self.gear == Gear::Lo { Gear::Hi } else { Gear::Lo };
            self.sfx.push(Sfx::Shift);
        }
    }

    pub fn kmh(&self) -> f32 {
        self.v * 3.6
    }

    fn rpm(&self) -> f32 {
        let r = match self.gear {
            Gear::Lo => self.v / 56.0,
            Gear::Hi => self.v / 92.0,
        };
        r.max(0.12)
    }

    fn engine_accel(&self) -> f32 {
        let v = self.v;
        match self.gear {
            Gear::Lo => 14.0 * (1.0 - v / 56.0),
            Gear::Hi if v < 30.0 => 2.0 + v * 0.2,
            Gear::Hi => 8.0 * (1.0 - (v - 30.0) / 62.0),
        }
    }

    fn wrap(&self, dz: f32) -> f32 {
        let l = self.track.length;
        (dz + l / 2.0).rem_euclid(l) - l / 2.0
    }

    fn message(&mut self, text: impl Into<String>, secs: f32) {
        self.message = Some((text.into(), secs));
    }

    pub fn update(&mut self, dt: f32, ctl: Controls) {
        if let Some((_, t)) = &mut self.message {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }

        let mut go = false;
        let moving = match &mut self.phase {
            Phase::Countdown { t, lights, go_at } => {
                *t += dt;
                let n = (t.floor() as u32).min(5);
                if n > *lights {
                    *lights = n;
                    self.sfx.push(Sfx::Light);
                }
                go = *t >= *go_at;
                false
            }
            Phase::Racing => {
                self.time += dt;
                true
            }
            Phase::Finished { t } => {
                *t += dt;
                true
            }
        };
        if go {
            self.phase = Phase::Racing;
            self.sfx.push(Sfx::Go);
            self.message("GO!", 1.2);
        }

        let ctl = if self.demo || self.finished_for().is_some() { self.ai_controls() } else { ctl };
        if moving {
            let previous = (self.d, self.x);
            self.update_player(dt, ctl);
            self.update_rivals(dt);
            self.collisions(previous);
            if !self.demo {
                self.check_laps();
            }
        } else {
            self.throttle = ctl.accel;
        }

        let target = if moving { self.rpm() } else if ctl.accel { 0.85 } else { 0.15 };
        self.rev += (target - self.rev) * (dt * 8.0).min(1.0);
    }

    fn update_player(&mut self, dt: f32, ctl: Controls) {
        let c = self.track.curve_at(self.d);
        self.grace = (self.grace - dt).max(0.0);
        if self.spin > 0.0 {
            self.spin -= dt;
            self.v = (self.v - 35.0 * dt).max(0.0);
            self.throttle = false;
            self.steer_vis = 0.0;
            if let Some(tx) = self.recover_x {
                // Slide back clear of the obstacle while spinning.
                self.x += (tx - self.x) * (dt * 4.0).min(1.0);
                if self.spin <= 0.0 {
                    self.x = tx;
                    self.recover_x = None;
                    self.grace = 1.5;
                }
            }
        } else {
            if self.auto_gear {
                if self.gear == Gear::Lo && self.v > 27.0 {
                    self.gear = Gear::Hi;
                } else if self.gear == Gear::Hi && self.v < 22.0 {
                    self.gear = Gear::Lo;
                }
            }
            let mut a = if ctl.accel { self.engine_accel() } else { -(2.5 + self.v * 0.03) };
            // Gravity: climbing costs speed, descending adds it.
            a -= 6.5 * self.track.slope_at(self.d);
            if ctl.brake {
                a -= 28.0;
            }
            self.v = (self.v + a * dt).max(0.0);
            self.throttle = ctl.accel;
            let grip = (self.v / 25.0).min(1.0);
            self.x += ctl.steer * STEER_SPEED * grip * dt;
            self.steer_vis = ctl.steer;
        }
        // Centrifugal drift pushes the car to the outside of bends.
        self.x -= c * self.v * self.v * CENTRIFUGAL * dt;
        self.offroad = self.x.abs() > ROAD_HALF + 0.6;
        if self.offroad && self.v > 22.0 {
            self.v -= 40.0 * dt;
        }
        self.x = self.x.clamp(-(ROAD_HALF + 12.0), ROAD_HALF + 12.0);
        self.d += self.v * dt;
        self.bg += c * self.v * dt * 0.3;
    }

    fn ai_controls(&mut self) -> Controls {
        let c_ahead = self.track.curve_at(self.d + 30.0);
        let c_now = self.track.curve_at(self.d);
        let finished = self.finished_for().is_some();
        let mut target_v = if finished { 35.0 } else { 78.0 * (1.0 - c_ahead.abs() * 20.0).max(0.55) };

        // Pick a lane free of slower traffic.
        let blocked = |lane: f32, me: &Race| {
            me.rivals.iter().any(|r| {
                let dz = me.wrap(r.d - me.d);
                dz > -6.0 && dz < 35.0 && (r.x - lane).abs() < 2.4
            })
        };
        if blocked(self.ai_x, self) {
            if let Some(&l) = LANES.iter().find(|&&l| !blocked(l, self)) {
                self.ai_x = l;
            }
        }
        if finished {
            self.ai_x = 0.0;
        }
        if self.demo {
            for rival in &self.rivals {
                let gap = self.wrap(rival.d - self.d);
                let in_path = (rival.x - self.x).abs() < CAR_HALF_W * 2.0 + 0.5
                    || (rival.x - self.ai_x).abs() < CAR_HALF_W * 2.0 + 0.5;
                if gap > 0.0 && gap < 45.0 && in_path {
                    let following_distance = CAR_LEN + 3.0 + self.v * 0.45;
                    target_v = target_v.min((rival.v + (gap - following_distance) * 0.8).max(0.0));
                }
            }
        }

        let grip = (self.v / 25.0).clamp(0.2, 1.0);
        let ff = c_now * self.v * self.v * CENTRIFUGAL / (STEER_SPEED * grip);
        let fb = (self.ai_x - self.x) * 0.5;
        Controls {
            steer: (ff + fb).clamp(-1.0, 1.0),
            accel: self.v < target_v,
            brake: self.v > target_v + if self.demo { 1.5 } else { 8.0 },
        }
    }

    fn update_rivals(&mut self, dt: f32) {
        let snap: Vec<(f32, f32, f32)> = self
            .rivals
            .iter()
            .map(|r| (r.d, r.x, r.v))
            .chain(std::iter::once((self.d, self.x, self.v)))
            .collect();
        let track = self.track.clone();
        let l = track.length;
        let wrap = |dz: f32| (dz + l / 2.0).rem_euclid(l) - l / 2.0;
        let lane_free = |lane: f32, d: f32, skip: usize| {
            snap.iter().enumerate().all(|(j, &(od, ox, _))| {
                let dz = wrap(od - d);
                j == skip || !(dz > -8.0 && dz < 30.0 && (ox - lane).abs() < 2.4)
            })
        };

        for i in 0..self.rivals.len() {
            let (d, x) = (self.rivals[i].d, self.rivals[i].x);
            let c = track.curve_at(d + 20.0);
            let r = &mut self.rivals[i];
            let mut target_v = r.top * (1.0 - c.abs() * 15.0).max(0.6);

            // Nearest car ahead in our target lane.
            let mut blocker: Option<(f32, f32)> = None;
            for (j, &(od, ox, ov)) in snap.iter().enumerate() {
                if j == i {
                    continue;
                }
                let dz = wrap(od - d);
                let in_lane = (ox - r.target_x).abs() < 2.2 || (ox - x).abs() < 2.0;
                if dz > 0.0 && dz < 25.0 && in_lane && blocker.is_none_or(|(bd, _)| dz < bd) {
                    blocker = Some((dz, ov));
                }
            }
            if let Some((dz, bv)) = blocker {
                if bv < target_v {
                    let alt = LANES.iter().copied().filter(|&ln| (ln - r.target_x).abs() > 1.0).find(|&ln| lane_free(ln, d, i));
                    match alt {
                        Some(ln) => r.target_x = ln,
                        None if dz < 12.0 => target_v = bv * 0.97,
                        None => {}
                    }
                }
            }

            r.lane_timer -= dt;
            if r.lane_timer <= 0.0 {
                r.lane_timer = self.rng.gen_range(4.0..10.0);
                let ln = LANES[self.rng.gen_range(0..LANES.len())];
                if lane_free(ln, d, i) {
                    r.target_x = ln;
                }
            }

            let a = ((target_v - r.v) * 1.2).clamp(-18.0, 7.0);
            r.v = (r.v + a * dt).max(0.0);
            let diff = r.target_x - r.x;
            r.x += diff.signum() * (2.5 * dt).min(diff.abs());
            r.d += r.v * dt;
        }
    }

    fn crash(&mut self) {
        self.spin = 1.8;
        self.sfx.push(Sfx::Crash);
        if !self.demo {
            self.message("CRASH!", 1.4);
        }
    }

    fn collisions(&mut self, previous: (f32, f32)) {
        if self.spin > 0.0 {
            return;
        }
        let mut crash = false;
        let mut bump = false;
        let l = self.track.length;
        for r in &mut self.rivals {
            let dz = (r.d - self.d + l / 2.0).rem_euclid(l) - l / 2.0;
            let dx = r.x - self.x;
            if dz.abs() >= CAR_LEN || dx.abs() >= CAR_HALF_W * 2.0 {
                continue;
            }
            if dz > 0.0 {
                // We ran into the back of a rival.
                if self.v - r.v > 14.0 {
                    crash = true;
                    self.v = r.v * 0.3;
                } else {
                    bump = true;
                    self.v = r.v * 0.85;
                }
                self.d -= CAR_LEN - dz;
            } else {
                bump = true;
                r.v = r.v.min(self.v * 0.9);
                r.d -= CAR_LEN + dz;
            }
            let push = if dx >= 0.0 { 0.4 } else { -0.4 };
            self.x -= push;
            r.x += push;
        }

        if !crash && self.grace <= 0.0 && previous.1.abs().max(self.x.abs()) > ROAD_HALF + 1.0 {
            let mut impact: Option<f32> = None;
            for scenery in &self.track.scenery {
                let distance = previous.0 + self.wrap(scenery.d - previous.0);
                let Some((enter_d, leave_d)) = sweep_interval(previous.0, self.d, distance - 1.2, distance + 1.2) else { continue };
                for &(offset, half_width) in scenery.kind.solid() {
                    let center = scenery.x + offset;
                    let lower = center - half_width - CAR_HALF_W;
                    let upper = center + half_width + CAR_HALF_W;
                    let (lower, upper) = if center >= 0.0 {
                        (lower.max(ROAD_HALF + 1.0), upper)
                    } else {
                        (lower, upper.min(-ROAD_HALF - 1.0))
                    };
                    let Some((enter_x, leave_x)) = sweep_interval(previous.1, self.x, lower, upper) else { continue };
                    let enter = enter_d.max(enter_x);
                    if enter <= leave_d.min(leave_x) {
                        impact = Some(impact.map_or(enter, |earlier| earlier.min(enter)));
                    }
                }
            }
            if let Some(time) = impact {
                self.d = previous.0 + (self.d - previous.0) * time;
                self.x = previous.1 + (self.x - previous.1) * time;
                crash = true;
                self.v *= 0.1;
                self.recover_x = Some(self.x.signum() * (ROAD_HALF - 1.0));
            }
        }

        if crash {
            self.crash();
        } else if bump {
            self.sfx.push(Sfx::Bump);
        }
    }

    fn check_laps(&mut self) {
        if self.finished_for().is_some() {
            return;
        }
        let l = ((self.d / self.track.length).floor() as i64 + 1).max(1) as u32;
        if l <= self.lap {
            return;
        }
        self.lap = l;
        let lap_time = self.time - self.lap_start;
        self.lap_start = self.time;
        self.last_lap = Some(lap_time);
        if self.best_lap.is_none_or(|b| lap_time < b) {
            self.best_lap = Some(lap_time);
        }
        if l > self.laps {
            self.final_pos = self.position();
            self.phase = Phase::Finished { t: 0.0 };
            self.sfx.push(Sfx::Finish);
            self.message("FINISH!", 2.5);
        } else {
            self.sfx.push(Sfx::Lap);
            if l == self.laps {
                self.message("FINAL LAP", 2.0);
            } else {
                self.message(format!("LAP {l}"), 1.5);
            }
        }
    }

    pub fn set_player_color(&mut self, color: Rgb) {
        self.car_color = color;
        for (rival, team) in self.rivals.iter_mut().zip(TEAM_COLORS) {
            rival.color = rival_color(team, color);
        }
    }

    pub fn cam_d(&self) -> f32 {
        self.d - PLAYER_Z
    }

    pub fn sprites(&self, out: &mut Vec<SpriteInst>) {
        out.clear();
        let cam = self.cam_d();
        let l = self.track.length;
        let art = sprites::get();
        for s in &self.track.scenery {
            let z = (s.d - cam).rem_euclid(l);
            if z > 0.8 && z < 400.0 {
                out.push(SpriteInst {
                    z,
                    x: s.x,
                    width_m: s.kind.width(),
                    bmp: s.kind.bitmap(),
                    body: s.kind.body(),
                    flip: s.kind.flipped(),
                    skew: 0,
                    lift: 0.0,
                    shadow: false,
                });
            }
        }
        for r in &self.rivals {
            let z = (r.d - cam).rem_euclid(l);
            if z > 0.8 && z < 400.0 {
                let skew = ((r.target_x - r.x) * 2.0).clamp(-2.0, 2.0).round() as i32;
                out.push(SpriteInst {
                    z,
                    x: r.x,
                    width_m: 2.0,
                    bmp: &art.car_rear,
                    body: r.color,
                    flip: false,
                    skew,
                    lift: 0.0,
                    shadow: true,
                });
            }
        }

        let (bmp, flip, width_m) = if self.spin > 0.0 {
            match (self.spin * 10.0) as usize % 4 {
                0 => (&art.car_rear, false, 2.0),
                1 => (&art.car_side, false, 4.4),
                2 => (&art.car_front, false, 2.0),
                _ => (&art.car_side, true, 4.4),
            }
        } else {
            (&art.car_rear, false, 2.0)
        };
        let bumpy = self.offroad && self.v > 3.0;
        out.push(SpriteInst {
            z: PLAYER_Z,
            x: self.x,
            width_m,
            bmp,
            body: self.car_color,
            flip,
            skew: (self.steer_vis * 2.0).round() as i32,
            lift: if bumpy { ((self.time * 25.0) as i32 % 2) as f32 * 0.06 } else { 0.0 },
            shadow: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::all_tracks;

    #[test]
    fn chosen_paint_keeps_the_player_distinct_from_rivals() {
        let mut race = Race::new(Rc::new(all_tracks().remove(3)), 3, true, false);
        let original_track_color = race.track.palette.car;
        for color in crate::settings::CarColor::ALL {
            race.set_player_color(color.rgb());
            assert_eq!(race.car_color, color.rgb());
            assert_eq!(race.track.palette.car, original_track_color);
            for rival in &race.rivals {
                let distance = (rival.color.0 as i32 - race.car_color.0 as i32).abs()
                    + (rival.color.1 as i32 - race.car_color.1 as i32).abs()
                    + (rival.color.2 as i32 - race.car_color.2 as i32).abs();
                assert!(distance >= 120, "{} is too close to {:?}", color.label(), rival.color);
            }
        }
    }

    #[test]
    fn menu_demo_keeps_a_safe_gap_through_the_opening_grid() {
        for track in all_tracks() {
            let track = Rc::new(track);
            for frames_per_second in [20, 60, 120] {
                for seed in [3, 42, 79] {
                    let mut race = Race::new(track.clone(), 99, true, true);
                    race.rng = StdRng::seed_from_u64(seed);
                    for (index, rival) in race.rivals.iter_mut().enumerate() {
                        rival.top = 84.0 - index as f32 * 1.5 + race.rng.gen_range(-1.0..1.0);
                        rival.lane_timer = race.rng.gen_range(3.0..8.0);
                    }
                    for _ in 0..frames_per_second * 12 {
                        race.update(1.0 / frames_per_second as f32, Controls::default());
                        assert!(
                            !race.sfx.drain(..).any(|sound| matches!(sound, Sfx::Bump | Sfx::Crash)),
                            "{} at {frames_per_second} FPS, seed {seed}: menu camera jolted at {:.2}s", track.name, race.time
                        );
                    }
                    assert!(race.v > 20.0, "{}: demo should keep moving", track.name);
                }
            }
        }
    }

    #[test]
    fn player_races_still_apply_car_collisions() {
        let mut race = Race::new(Rc::new(all_tracks().remove(3)), 3, true, false);
        race.phase = Phase::Racing;
        race.d = 160.0;
        race.x = 0.0;
        race.v = 40.0;
        race.rivals.truncate(1);
        race.rivals[0].d = race.d + 3.0;
        race.rivals[0].x = race.x;
        race.rivals[0].v = 20.0;
        race.collisions((race.d, race.x));
        assert!(race.spin > 0.0);
        assert!(race.d < 160.0);
        assert_eq!(race.v, 6.0);
        assert!(race.sfx.iter().any(|sound| matches!(sound, Sfx::Crash)));
    }

    #[test]
    fn scenery_collisions_follow_the_movement_path() {
        for distance in [100.0, 0.0] {
            for frames_per_second in [20, 30, 60, 120] {
                let mut track = all_tracks().remove(3);
                track.scenery = vec![crate::track::Scenery { d: distance, x: 7.0, kind: crate::track::Prop::Tree }];
                let mut race = Race::new(Rc::new(track), 3, true, false);
                race.phase = Phase::Racing;
                race.rivals.clear();
                race.d = distance - 1.5;
                race.x = 7.0;
                race.v = 80.0;
                for _ in 0..frames_per_second / 10 {
                    race.update(1.0 / frames_per_second as f32, Controls { accel: true, ..Controls::default() });
                }
                assert!(race.sfx.iter().any(|sound| matches!(sound, Sfx::Crash)), "{frames_per_second} FPS at {distance}m");
            }
        }
    }

    #[test]
    fn scenery_collisions_require_simultaneous_overlap_and_choose_the_first_hit() {
        for (start_x, end_x, end_d, expected_crash) in [
            (4.0, 4.0, 102.0, false),
            (7.0, 1.0, 102.0, false),
            (4.0, 7.0, 102.0, true),
            (7.0, 7.0, 97.0, false),
            (7.0, 7.0, 102.0, true),
        ] {
            let mut track = all_tracks().remove(3);
            track.scenery = vec![
                crate::track::Scenery { d: 101.0, x: 7.0, kind: crate::track::Prop::Tree },
                crate::track::Scenery { d: 100.0, x: 7.0, kind: crate::track::Prop::Tree },
            ];
            let mut race = Race::new(Rc::new(track), 3, true, false);
            race.rivals.clear();
            race.d = end_d;
            race.x = end_x;
            race.v = 80.0;
            race.collisions((98.0, start_x));
            assert_eq!(race.spin > 0.0, expected_crash, "path ({start_x}, {end_x}, {end_d})");
            if start_x == 7.0 && end_x == 7.0 && expected_crash {
                assert!((race.d - 98.8).abs() < 0.001);
            }
        }
    }

    #[test]
    fn crashing_into_scenery_can_be_recovered_from() {
        for track in all_tracks() {
            let track = Rc::new(track);
            for s in track.scenery.iter().filter(|s| s.d > 100.0) {
                let Some(&(off, _)) = s.kind.solid().first() else { continue };
                let mut race = Race::new(track.clone(), 3, true, false);
                race.phase = Phase::Racing;
                // Drive flat out straight at a solid roadside object.
                race.d = s.d - 30.0;
                race.x = s.x + off;
                race.v = 60.0;
                let mut crashes = 0;
                for _ in 0..(60 * 10) {
                    // After the crash, steer back towards the middle like a player would.
                    let steer = if crashes > 0 { (-race.x * 0.5).clamp(-1.0, 1.0) } else { 0.0 };
                    race.update(1.0 / 60.0, Controls { steer, accel: true, brake: false });
                    crashes += race.sfx.drain(..).filter(|s| matches!(s, Sfx::Crash)).count();
                }
                let at = format!("{} at d={:.0}", track.name, s.d);
                assert!(crashes <= 1, "{at}: crashed {crashes} times, the car should recover");
                assert!(race.d > s.d + 50.0, "{at}: the car should drive on past the obstacle");
            }
        }
    }

    #[test]
    fn ai_driver_finishes_every_track() {
        for track in all_tracks() {
            let name = track.name;
            let mut race = Race::new(Rc::new(track), 2, true, false);
            let dt = 1.0 / 60.0;
            let mut crashes = 0;
            for _ in 0..(60 * 400) {
                let ctl = race.ai_controls();
                race.update(dt, ctl);
                crashes += race.sfx.drain(..).filter(|s| matches!(s, Sfx::Crash)).count();
                if race.finished_for().is_some() {
                    break;
                }
            }
            assert!(race.finished_for().is_some(), "{name}: did not finish");
            println!(
                "{name}: time {:.1}s best lap {:.1}s pos {} crashes {crashes}",
                race.time,
                race.best_lap.unwrap(),
                race.final_pos
            );
        }
    }
}
