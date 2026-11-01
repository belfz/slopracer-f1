use super::{draw_distant_ground, draw_halo, hash, hashf, FrameBuf, Palette, Rgb, View};

pub(super) fn draw(frame: &mut FrameBuf, view: &View, palette: &Palette, background: f32) {
    let width = view.w as f32;
    let horizon = view.hz;
    let moon_x = (width * (0.24 - background * 0.08)).rem_euclid(width * 1.4) - width * 0.1;
    let moon_y = horizon * 0.26;
    let radius = view.h as f32 * 0.037;
    let moon = palette.sun.unwrap_or(Rgb(202, 225, 245));
    draw_halo(frame, (moon_x, moon_y), radius * 2.7, view.aspect, Rgb(90, 145, 215), 0.18);
    frame.ellipse(moon_x, moon_y, radius / view.aspect, radius, moon, 1.0);
    for (offset_x, offset_y, size) in [(-0.25, -0.2, 0.22), (0.3, 0.3, 0.16), (0.05, 0.5, 0.12)] {
        frame.ellipse(moon_x + offset_x * radius / view.aspect, moon_y + offset_y * radius, radius * size / view.aspect, radius * size, Rgb(139, 171, 203), 0.4);
    }
    for (center, color) in [(0.18, Rgb(180, 40, 135)), (0.65, Rgb(35, 115, 175)), (0.92, Rgb(155, 55, 185))] {
        draw_halo(frame, (width * center, horizon * 1.12), horizon * 0.9, view.aspect, color, 0.12);
    }
    for (layer, columns, scroll, base_color) in [
        (0u32, 38.0, 0.22, Rgb(27, 30, 58)),
        (1u32, 23.0, 0.42, Rgb(16, 21, 41)),
    ] {
        for horizontal in 0..view.w {
            let position = (horizontal as f32 / width + background * scroll) * columns;
            let block = position.floor() as i64 as u32;
            let local = position.rem_euclid(1.0);
            let seed = block.wrapping_mul(31).wrapping_add(layer * 173);
            let height = if layer == 0 { 0.14 + hashf(seed) * 0.32 } else { 0.16 + hashf(seed) * 0.47 };
            let setback = if !(0.12..=0.88).contains(&local) { 0.05 } else { height };
            let roof = horizon * (1.0 - setback);
            let building_width = width / columns;
            let local_x = (local * building_width) as u32;
            let color = base_color.lerp(palette.fog, if layer == 0 { 0.25 } else { 0.0 });
            for vertical in roof.max(0.0) as usize..horizon.ceil() as usize {
                let local_y = vertical as u32 - roof as u32;
                let window = local_x % 5 == 2 && local_y % 7 < 2 && local_y > 4;
                let lit = !hash(seed ^ (local_x / 5).wrapping_mul(71) ^ (local_y / 7).wrapping_mul(131)).is_multiple_of(4);
                let light = [Rgb(95, 162, 193), Rgb(220, 166, 112), Rgb(170, 77, 159)][(seed % 3) as usize];
                frame.set(horizontal as i32, vertical as i32, if window && lit { light.scale(if layer == 0 { 0.48 } else { 0.7 }) } else { color });
            }
            if setback == height && layer == 1 && hash(seed).is_multiple_of(3) {
                frame.set(horizontal as i32, roof as i32, Rgb(135, 68, 155));
            }
        }
        if layer == 0 {
            let tower_x = (width * (0.73 - background * 0.22)).rem_euclid(width * 1.5) - width * 0.12;
            draw_tower(frame, view, tower_x);
        }
    }
    for vertical in (horizon * 0.93) as usize..horizon.ceil() as usize {
        let opacity = (vertical as f32 / horizon - 0.93) * 2.2;
        for horizontal in 0..view.w {
            frame.blend(horizontal as i32, vertical as i32, Rgb(111, 49, 126), opacity);
        }
    }
    draw_distant_ground(frame, view, palette);
}

fn line(frame: &mut FrameBuf, start: (f32, f32), end: (f32, f32), color: Rgb) {
    let count = (end.0 - start.0).abs().max((end.1 - start.1).abs()).ceil().max(1.0) as usize;
    for index in 0..=count {
        let progress = index as f32 / count as f32;
        frame.set((start.0 + (end.0 - start.0) * progress) as i32, (start.1 + (end.1 - start.1) * progress) as i32, color);
    }
}

fn draw_tower(frame: &mut FrameBuf, view: &View, center: f32) {
    let base = view.hz * 0.98;
    let height = view.hz * 0.79;
    let half_width = height * 0.17 / view.aspect;
    let light = Rgb(242, 111, 112);
    draw_halo(frame, (center, base - height * 0.6), height * 0.23, view.aspect, Rgb(235, 65, 115), 0.09);
    line(frame, (center, base - height * 1.1), (center, base - height * 0.66), Rgb(238, 180, 167));
    line(frame, (center - half_width, base), (center, base - height), light);
    line(frame, (center + half_width, base), (center, base - height), light);
    for level in 0..8 {
        let lower = level as f32 / 8.0;
        let upper = (level + 1) as f32 / 8.0;
        let lower_width = half_width * (1.0 - lower);
        let upper_width = half_width * (1.0 - upper);
        let lower_y = base - height * lower;
        let upper_y = base - height * upper;
        line(frame, (center - lower_width, lower_y), (center + lower_width, lower_y), light.scale(0.7));
        line(frame, (center - lower_width, lower_y), (center + upper_width, upper_y), light.scale(0.55));
        line(frame, (center + lower_width, lower_y), (center - upper_width, upper_y), light.scale(0.55));
    }
    frame.fill_rect((center - half_width * 0.55) as i32, (base - height * 0.57) as i32, (half_width * 1.1).max(2.0) as i32, (height * 0.035).max(1.0) as i32, Rgb(245, 172, 152));
}
