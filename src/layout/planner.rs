use std::collections::HashMap;

use bevy::math::Vec2;

use super::{Layout, MAX_SPEED, SPACING, hash_angle, parent_path, slot_count};
use crate::timeline::{Event, Kind};

const LOOK_AHEAD_COMMITS: usize = 128;
const GAP: f32 = 0.3;
pub(super) const RELEASE_RADIUS: f32 = 0.05;

#[derive(Clone, Debug)]
pub(super) struct Plan {
    occupancy: Vec<Vec<(usize, usize)>>,
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
            occupancy,
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
    pub(super) fn relax_planned(&mut self, dt: f32) {
        let mut live: Vec<_> = self.dirs().map(|(index, _)| index).collect();
        let mut radii = vec![0.0; self.dirs.len()];
        for &index in &live {
            let dir = &mut self.dirs[index];
            let target = if index != 0 && dir.files == 0 && dir.children == 0 {
                0.0
            } else {
                dir.radius().max(
                    self.plan
                        .as_ref()
                        .expect("planned layout has a plan")
                        .radius(index),
                )
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
        live.sort_by(|&a, &b| {
            (positions[a].x - radii[a])
                .total_cmp(&(positions[b].x - radii[b]))
                .then(a.cmp(&b))
        });
        let mut correction = vec![Vec2::ZERO; self.dirs.len()];
        for (offset, &a) in live.iter().enumerate() {
            for &b in &live[offset + 1..] {
                if positions[b].x - radii[b] > positions[a].x + radii[a] + GAP {
                    break;
                }
                let delta = positions[b] - positions[a];
                let reach = radii[a] + radii[b] + GAP;
                let distance = delta.length();
                if distance >= reach {
                    continue;
                }
                let unit = delta
                    .normalize_or(Vec2::from_angle(hash_angle(&self.dirs[b].path)));
                let push = unit * (reach - distance) * 0.5;
                correction[a] -= push * if b == 0 { 2.0 } else { 1.0 };
                correction[b] += push * if a == 0 { 2.0 } else { 1.0 };
            }
        }
        for &index in live.iter() {
            if index != 0 {
                positions[index] += correction[index] * 0.8;
            }
        }
    }

    /// Reserves peak concurrent occupancy over the next 128 commits, excluding past peaks.
    pub(crate) fn planned(events: &[Event]) -> Self {
        let mut layout = Self::default();
        let plan = Plan::new(&mut layout, events);
        for (index, dir) in layout.dirs.iter_mut().enumerate() {
            dir.live = index == 0;
            dir.activated = index == 0;
            dir.children = 0;
        }
        layout.plan = Some(plan);
        layout
    }

    pub(crate) fn begin_commit(&mut self) {
        if let Some(plan) = &mut self.plan {
            plan.next_commit += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::FileChange;

    const DT: f32 = 1.0 / 60.0;

    fn event(changes: &[(&str, Kind)]) -> Event {
        Event {
            author: "Author".to_owned(),
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
        let plan = layout.plan.as_ref().unwrap();
        assert_eq!(plan.radius(old), SPACING + 0.4);
        assert_eq!(plan.radius(late), 0.4);
        assert_eq!(layout.dirs().count(), 1);
        for _ in 0..=LOOK_AHEAD_COMMITS {
            layout.begin_commit();
        }
        let plan = layout.plan.as_ref().unwrap();
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
        let plan = layout.plan.as_ref().unwrap();
        assert_eq!(plan.radius(layout.dir_index["b"]), SPACING + 0.4);
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
