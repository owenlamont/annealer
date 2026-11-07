use std::collections::HashMap;

use bevy::math::Vec2;

use super::{Layout, MAX_SPEED, hash_angle, slot_count};

const GAP: f32 = 0.3;

fn nearby_pairs(
    positions: &[Vec2],
    radii: &[f32],
    live: &mut [usize],
    mut visit: impl FnMut(usize, usize, Vec2, f32),
) {
    live.sort_by(|&a, &b| {
        (positions[a].x - radii[a])
            .total_cmp(&(positions[b].x - radii[b]))
            .then(a.cmp(&b))
    });
    if live.len() <= 32 {
        for (offset, &a) in live.iter().enumerate() {
            for &b in &live[offset + 1..] {
                if positions[b].x - radii[b] > positions[a].x + radii[a] + GAP {
                    break;
                }
                visit(a, b, positions[b] - positions[a], radii[a] + radii[b] + GAP);
            }
        }
        return;
    }
    let width = live.iter().map(|&index| radii[index]).fold(0.0, f32::max) * 2.0 + GAP;
    let mut cells: HashMap<_, Vec<_>> = HashMap::new();
    for (rank, &index) in live.iter().enumerate() {
        cells
            .entry((positions[index] / width).floor().as_ivec2())
            .or_default()
            .push((rank, index, positions[index], radii[index]));
    }
    for (rank, &a) in live.iter().enumerate() {
        let cell = (positions[a] / width).floor().as_ivec2();
        for offset in
            (-1..=1).flat_map(|y| (-1..=1).map(move |x| bevy::math::IVec2::new(x, y)))
        {
            let Some(neighbors) = cells.get(&(cell + offset)) else {
                continue;
            };
            let first = neighbors.partition_point(|&(other, ..)| other <= rank);
            for &(.., b, position, radius) in &neighbors[first..] {
                if position.x - radius > positions[a].x + radii[a] + GAP {
                    break;
                }
                let delta = position - positions[a];
                let reach = radii[a] + radius + GAP;
                if delta.y.abs() <= reach {
                    visit(a, b, delta, reach);
                }
            }
        }
    }
}

impl Layout {
    pub(super) fn relax_unplanned(&mut self, dt: f32) {
        let mut live: Vec<_> = self.dirs().map(|(index, _)| index).collect();
        let mut radii = vec![0.0; self.dirs.len()];
        for &index in &live {
            let dir = &mut self.dirs[index];
            let target = if index != 0 && dir.files == 0 && dir.children == 0 {
                0.0
            } else {
                dir.radius()
            };
            dir.reserve += (target - dir.reserve) * (1.0 - (-dt / 0.2).exp());
            if dir.files > 0 {
                dir.reserve = dir.reserve.max(dir.radius());
            }
            radii[index] = dir.reserve;
        }
        let mut positions: Vec<_> = self.dirs.iter().map(|dir| dir.position).collect();
        let mut pull = vec![Vec2::ZERO; self.dirs.len()];
        for &index in live.iter().skip(1) {
            let parent = self.dirs[index]
                .parent
                .expect("non-root directory has parent");
            let delta = positions[index] - positions[parent];
            let rest = radii[index] + radii[parent] + GAP;
            let attraction = delta.normalize_or_zero()
                * (rest - delta.length())
                * (1.0 - (-dt * 0.6).exp());
            pull[index] += attraction / slot_count(self.dirs[index].children + 1);
            if parent != 0 {
                pull[parent] -= attraction / slot_count(self.dirs[parent].children + 1);
            }
        }
        for &index in &live {
            positions[index] += pull[index];
        }
        for _ in 0..3 {
            self.separate_discs(&mut positions, &radii, &mut live);
        }
        for &index in &live {
            if index != 0 {
                let dir = &mut self.dirs[index];
                dir.position +=
                    (positions[index] - dir.position).clamp_length_max(MAX_SPEED * dt);
            }
        }
    }

    fn separate_discs(
        &self,
        positions: &mut [Vec2],
        radii: &[f32],
        live: &mut [usize],
    ) {
        let mut correction = vec![Vec2::ZERO; self.dirs.len()];
        let mut directions = vec![Vec2::ZERO; self.dirs.len()];
        for &index in live.iter() {
            directions[index] = Vec2::from_angle(hash_angle(&self.dirs[index].path));
        }
        nearby_pairs(positions, radii, live, |a, b, delta, reach| {
            let distance = delta.length();
            if distance >= reach {
                return;
            }
            let inverse = distance.recip();
            let unit = if inverse.is_finite() && inverse > 0.0 {
                delta * inverse
            } else {
                directions[b]
            };
            let push = unit * (reach - distance) * 0.5;
            correction[a] -= push * if b == 0 { 2.0 } else { 1.0 };
            correction[b] += push * if a == 0 { 2.0 } else { 1.0 };
        });
        for &index in live.iter() {
            if index != 0 {
                positions[index] += correction[index] * 0.8;
            }
        }
    }
}
