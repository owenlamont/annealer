use std::{collections::HashMap, sync::Arc};

use bevy::math::Vec2;

use super::{Layout, MAX_SPEED, SPACING, hash_angle, parent_path, slot_count};
use crate::timeline::{Event, Kind};

const LOOK_AHEAD_COMMITS: usize = 128;
const GAP: f32 = 0.3;
pub(super) const RELEASE_RADIUS: f32 = 0.05;

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

#[derive(Clone, Debug, Default)]
pub(super) struct Plan {
    occupancy: Arc<[Vec<(usize, usize)>]>,
    next_commit: usize,
}

impl Plan {
    fn new(layout: &mut Layout, events: &[Event]) -> Self {
        let mut membership: HashMap<&str, usize> = HashMap::new();
        let mut counts = vec![0_usize];
        let mut occupancy = vec![Vec::new()];
        for (event_index, event) in events.iter().enumerate() {
            for change in &event.changes {
                let removed = match &change.kind {
                    Kind::Deleted => Some(change.path.as_str()),
                    Kind::Moved { from } => Some(from.as_str()),
                    _ => None,
                };
                if let Some(path) = removed
                    && let Some(dir) = membership.remove(path)
                {
                    counts[dir] -= 1;
                    occupancy[dir].push((event_index + 1, counts[dir]));
                }
                if change.kind == Kind::Deleted
                    || membership.contains_key(change.path.as_str())
                {
                    continue;
                }
                let dir = layout.ensure_dir(parent_path(&change.path));
                counts.resize(layout.dir_count(), 0);
                occupancy.resize_with(layout.dir_count(), Vec::new);
                membership.insert(change.path.as_str(), dir);
                counts[dir] += 1;
                occupancy[dir].push((event_index + 1, counts[dir]));
            }
        }
        Self {
            occupancy: occupancy.into(),
            next_commit: 0,
        }
    }

    pub(super) fn radius(&self, dir: usize) -> f32 {
        let Some(history) = self.occupancy.get(dir) else {
            return 0.4;
        };
        let first_future =
            history.partition_point(|&(commit, _)| commit <= self.next_commit);
        let current = first_future
            .checked_sub(1)
            .map_or(0, |index| history[index].1);
        let peak = history[first_future..]
            .iter()
            .take_while(|&&(commit, _)| commit <= self.next_commit + LOOK_AHEAD_COMMITS)
            .map(|&(_, count)| count)
            .max()
            .unwrap_or(0)
            .max(current);
        SPACING * slot_count(peak).sqrt() + 0.4
    }
}

impl Layout {
    pub(super) fn relax(&mut self, dt: f32) {
        let mut live: Vec<_> = self.dirs().map(|(index, _)| index).collect();
        let mut radii = vec![0.0; self.dirs.len()];
        for &index in &live {
            let dir = &mut self.dirs[index];
            let target = if index != 0 && dir.files == 0 && dir.children == 0 {
                0.0
            } else {
                dir.radius().max(self.plan.radius(index))
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

    /// Reserves peak concurrent occupancy over the next 128 commits, excluding past peaks.
    pub(crate) fn planned(events: &[Event]) -> Self {
        let mut layout = Self::empty();
        let plan = Plan::new(&mut layout, events);
        for (index, dir) in layout.dirs.iter_mut().enumerate() {
            dir.live = index == 0;
            dir.activated = index == 0;
            dir.children = 0;
        }
        layout.plan = plan;
        layout
    }

    pub(crate) fn begin_commit(&mut self) {
        self.plan.next_commit += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::FileChange;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn grid_matches_brute_force_separation_and_ignores_input_order() {
        let mut layout = Layout::empty();
        for index in 1..96 {
            layout.ensure_dir(&format!("dir-{index}"));
        }
        let positions: Vec<_> = (0_i16..96)
            .map(|index| {
                Vec2::new(f32::from(index % 12 - 6), f32::from(index / 12 - 4)) * 2.34
            })
            .collect();
        let radii: Vec<_> = (0..96)
            .map(|index| [0.0, 0.4, 1.02, 4.0][index % 4])
            .collect();
        let mut live: Vec<_> = (0..96).filter(|index| index % 7 != 6).collect();
        let mut original = positions;
        original[0] = Vec2::ZERO;
        original[1] = Vec2::ZERO;
        original[2] = Vec2::new(0.0, radii[0] + radii[2] + GAP);
        let mut expected = original.clone();
        let mut actual = original.clone();
        let mut reversed = original.clone();
        for _ in 0..3 {
            live.sort_by(|&a, &b| {
                (expected[a].x - radii[a])
                    .total_cmp(&(expected[b].x - radii[b]))
                    .then(a.cmp(&b))
            });
            let mut pairs = Vec::new();
            let mut correction = vec![Vec2::ZERO; expected.len()];
            for (a, b) in live.iter().enumerate().flat_map(|(offset, &a)| {
                live[offset + 1..].iter().map(move |&b| (a, b))
            }) {
                let delta = expected[b] - expected[a];
                let reach = radii[a] + radii[b] + GAP;
                let distance = delta.length();
                if distance >= reach {
                    continue;
                }
                pairs.push((a, b));
                let unit = delta
                    .normalize_or(Vec2::from_angle(hash_angle(&layout.dirs[b].path)));
                let push = unit * (reach - distance) * 0.5;
                correction[a] -= push * if b == 0 { 2.0 } else { 1.0 };
                correction[b] += push * if a == 0 { 2.0 } else { 1.0 };
            }
            let mut found = Vec::new();
            live.reverse();
            nearby_pairs(&expected, &radii, &mut live, |a, b, _, _| {
                if expected[a].distance(expected[b]) < radii[a] + radii[b] + GAP {
                    found.push((a, b));
                }
            });
            assert_ne!(pairs, []);
            found.sort_unstable();
            pairs.sort_unstable();
            assert_eq!(found, pairs);
            for &index in live.iter().filter(|&&index| index != 0) {
                expected[index] += correction[index] * 0.8;
            }
            live.reverse();
            layout.separate_discs(&mut actual, &radii, &mut live);
            live.reverse();
            layout.separate_discs(&mut reversed, &radii, &mut live);
            assert_eq!(actual, reversed);
            for (actual, expected) in actual.iter().zip(&expected) {
                assert!(actual.distance(*expected) < 0.000_02);
            }
            assert_eq!(actual[0], original[0]);
        }
    }

    #[test]
    #[ignore = "release-mode layout measurement"]
    fn layout_step_cost() {
        use std::{hint::black_box, time::Instant};

        for (children, warm_up) in [(12, 10), (3_000, 10), (3_000, 120)] {
            let mut history = event(&[]);
            history.changes = (0..children)
                .map(|index| FileChange {
                    path: format!("child-{index}/file"),
                    kind: Kind::Added,
                    insertions: 1,
                    removals: 0,
                })
                .collect();
            let mut layout = Layout::planned(std::slice::from_ref(&history));
            layout.begin_commit();
            for change in &history.changes {
                layout.add(&change.path, 0.0, 0);
            }
            for _ in 0..warm_up {
                layout.step(DT);
            }
            let positions: Vec<_> =
                layout.dirs.iter().map(|dir| dir.position).collect();
            let radii: Vec<_> = layout.dirs.iter().map(|dir| dir.reserve).collect();
            let mut live: Vec<_> = layout.dirs().map(|(index, _)| index).collect();
            let mut candidates = 0;
            let mut overlaps = 0;
            nearby_pairs(&positions, &radii, &mut live, |a, b, _, _| {
                candidates += 1;
                overlaps += usize::from(
                    positions[a].distance(positions[b]) < radii[a] + radii[b] + GAP,
                );
            });
            println!("candidates={candidates}, overlaps={overlaps}");
            let mut step = std::time::Duration::ZERO;
            let mut relax = std::time::Duration::ZERO;
            let mut separation = std::time::Duration::ZERO;
            for _ in 0..100 {
                let mut sample = layout.clone();
                let start = Instant::now();
                sample.step(black_box(DT));
                step += start.elapsed();
                black_box(sample);

                let mut sample = layout.clone();
                let start = Instant::now();
                sample.relax(black_box(DT));
                relax += start.elapsed();
                black_box(sample);

                let mut positions: Vec<_> =
                    layout.dirs.iter().map(|dir| dir.position).collect();
                let radii: Vec<_> = layout.dirs.iter().map(|dir| dir.reserve).collect();
                let mut live: Vec<_> = layout.dirs().map(|(index, _)| index).collect();
                let start = Instant::now();
                for _ in 0..3 {
                    layout.separate_discs(&mut positions, &radii, &mut live);
                }
                separation += start.elapsed();
                black_box(positions);
            }
            println!(
                "children={children}, warm_up={warm_up}: step={:.3} ms relax={:.3} ms separation_3x={:.3} ms (100 snapshot samples)",
                step.as_secs_f64() * 10.0,
                relax.as_secs_f64() * 10.0,
                separation.as_secs_f64() * 10.0,
            );
        }
    }

    fn event(changes: &[(&str, Kind)]) -> Event {
        Event {
            author: crate::timeline::Identity {
                name: b"Author".to_vec(),
                email: Vec::new(),
            },
            contributor: None,
            seconds: 0,
            changes: changes
                .iter()
                .map(|(path, kind)| FileChange {
                    path: (*path).to_owned(),
                    kind: kind.clone(),
                    insertions: 1,
                    removals: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn distant_growth_and_deleted_paths_do_not_reserve_lifetime_capacity() {
        let mut events = vec![event(&[]); LOOK_AHEAD_COMMITS * 3];
        events[0] = event(&[("old/a", Kind::Added)]);
        events[1] = event(&[("old/a", Kind::Deleted), ("old/b", Kind::Added)]);
        events[2] = event(&[("old/b", Kind::Deleted)]);
        events[LOOK_AHEAD_COMMITS * 2] =
            event(&[("late/a", Kind::Added), ("late/b", Kind::Added)]);
        let mut layout = Layout::planned(&events);
        let old = layout.dir_index["old"];
        let late = layout.dir_index["late"];
        let plan = &layout.plan;
        assert_eq!(plan.radius(old), SPACING + 0.4);
        assert_eq!(plan.radius(late), 0.4);
        assert_eq!(layout.dirs().count(), 1);
        for _ in 0..=LOOK_AHEAD_COMMITS {
            layout.begin_commit();
        }
        let plan = &layout.plan;
        assert_eq!(plan.radius(old), 0.4);
        assert!(plan.radius(late) > SPACING + 0.4);
    }

    #[test]
    fn replacing_a_move_destination_does_not_double_count_occupancy() {
        let events = [
            event(&[("a/file", Kind::Added), ("b/file", Kind::Added)]),
            event(&[(
                "b/file",
                Kind::Moved {
                    from: "a/file".to_owned(),
                },
            )]),
        ];
        let layout = Layout::planned(&events);
        let plan = &layout.plan;
        assert_eq!(plan.radius(layout.dir_index["b"]), SPACING + 0.4);
    }

    #[test]
    fn cloned_layouts_share_occupancy_but_advance_cursors_independently() {
        let events = [event(&[("a/file", Kind::Added)])];
        let layout = Layout::planned(&events);
        let mut replay = layout.clone();
        assert!(Arc::ptr_eq(&layout.plan.occupancy, &replay.plan.occupancy));
        replay.begin_commit();
        assert_eq!(layout.plan.next_commit, 0);
        assert_eq!(replay.plan.next_commit, 1);
    }

    #[test]
    fn revival_after_parent_growth_spawns_outside_the_live_reservation() {
        let mut events = vec![event(&[]); 301];
        events[0] = event(&[("a/x/file", Kind::Added)]);
        events[1] = event(&[("a/x/file", Kind::Deleted)]);
        events[150].changes = (0..200)
            .map(|index| FileChange {
                path: format!("a/{index}"),
                kind: Kind::Added,
                insertions: 1,
                removals: 0,
            })
            .collect();
        events[300] = event(&[("a/x/file", Kind::Added)]);
        let mut layout = Layout::planned(&events);
        let child = layout.dir_index["a/x"];
        let parent = layout.dir_index["a"];
        for event in &events[..300] {
            layout.begin_commit();
            for change in &event.changes {
                match change.kind {
                    Kind::Added => {
                        layout.add(&change.path, 0.0, 0);
                    }
                    Kind::Deleted => {
                        layout.remove(&change.path, 0.0, 0);
                    }
                    _ => unreachable!("fixture contains only additions and deletions"),
                }
            }
            for _ in 0..30 {
                layout.step(DT);
            }
        }
        assert!(!layout.dir(child).live);
        assert!(
            layout
                .dir(child)
                .position
                .distance(layout.dir(parent).position)
                < layout.dir(parent).radius()
        );
        let parent_position = layout.dir(parent).position;
        layout.begin_commit();
        layout.add("a/x/file", 0.0, 0);
        assert_eq!(layout.dir(parent).position, parent_position);
        assert!(
            layout.dir(child).position.distance(parent_position)
                >= layout.dir(parent).reserve.max(layout.dir(parent).radius())
                    + layout.dir(child).radius()
        );
    }

    #[test]
    fn moving_a_sole_nested_file_releases_space_gradually_without_jumps() {
        let events = [
            event(&[("a/deep/file", Kind::Added)]),
            event(&[(
                "b/file",
                Kind::Moved {
                    from: "a/deep/file".to_owned(),
                },
            )]),
        ];
        let mut layout = Layout::planned(&events);
        layout.begin_commit();
        let id = layout.add("a/deep/file", 0.0, 0);
        layout.settle(120, DT);
        let source = layout.dir_index["a/deep"];
        let before = layout.dirs[source].reserve;
        let file_position = layout.file(id).unwrap().position;
        layout.begin_commit();
        assert_eq!(layout.rename("a/deep/file", "b/file", 0), id);
        assert_eq!(layout.file(id).unwrap().position, file_position);
        layout.step(DT);
        assert!(layout.dir(source).live);
        assert!((0.0..before).contains(&layout.dirs[source].reserve));
        let mut replay = layout.clone();
        for _ in 0..360 {
            let positions: Vec<_> =
                layout.dirs.iter().map(|dir| dir.position).collect();
            layout.step(DT);
            replay.step(DT);
            for (dir, previous) in layout.dirs.iter().zip(positions) {
                assert!(dir.position.distance(previous) <= MAX_SPEED * DT + 1e-5);
            }
            assert_eq!(
                layout
                    .dirs
                    .iter()
                    .map(|dir| dir.position)
                    .collect::<Vec<_>>(),
                replay
                    .dirs
                    .iter()
                    .map(|dir| dir.position)
                    .collect::<Vec<_>>()
            );
        }
        assert!(!layout.dir(source).live);
        assert!(!layout.dir(layout.dir_index["a"]).live);
        assert!(layout.dir(layout.dir_index["b"]).live);
    }

    #[test]
    fn growing_siblings_settle_without_disc_overlap() {
        let events = [event(&[
            ("a/one", Kind::Added),
            ("a/two", Kind::Added),
            ("b/one", Kind::Added),
            ("b/deep/one", Kind::Added),
        ])];
        let mut layout = Layout::planned(&events);
        layout.begin_commit();
        for change in &events[0].changes {
            layout.add(&change.path, 0.0, 0);
        }
        layout.settle(300, DT);
        let dirs: Vec<_> = layout.dirs().map(|(_, dir)| dir).collect();
        for (index, a) in dirs.iter().enumerate() {
            for b in &dirs[index + 1..] {
                assert!(
                    a.position.distance(b.position) >= a.radius() + b.radius(),
                    "{} overlaps {}",
                    a.path,
                    b.path
                );
            }
        }
    }

    #[test]
    fn deleting_inner_slots_reclaims_the_outer_footprint_without_teleporting() {
        let events = [
            event(&[
                ("a/0", Kind::Added),
                ("a/1", Kind::Added),
                ("a/2", Kind::Added),
                ("a/3", Kind::Added),
            ]),
            event(&[
                ("a/0", Kind::Deleted),
                ("a/1", Kind::Deleted),
                ("a/2", Kind::Deleted),
            ]),
        ];
        let mut layout = Layout::planned(&events);
        layout.begin_commit();
        for change in &events[0].changes {
            layout.add(&change.path, 0.0, 0);
        }
        layout.settle(120, DT);
        let id = layout.file_index["a/3"];
        let position = layout.file(id).unwrap().position;
        let dir = layout.file(id).unwrap().dir;
        layout.begin_commit();
        for path in ["a/0", "a/1", "a/2"] {
            layout.remove(path, 0.0, 0);
        }
        for _ in 0..120 {
            let previous = layout.file(id).unwrap().position;
            layout.step(DT);
            assert!(layout.file(id).unwrap().position.distance(previous) < 0.5);
        }
        assert_eq!(layout.file(id).unwrap().slot, 0);
        assert_eq!(layout.dir(dir).slots.len(), 1);
        assert!(layout.file(id).unwrap().position.distance(position) > 0.1);
        assert!((layout.dirs[dir].reserve - (SPACING + 0.4)).abs() < 0.01);
    }
}
