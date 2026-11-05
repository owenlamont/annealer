use std::collections::BTreeSet;

use bevy::math::Vec2;

use super::{GAP, Layout, SPACING, parent_path, slot_count};
use crate::timeline::{Event, Kind};

impl Layout {
    /// Reserves disjoint subtree rectangles for all paths, including fading files.
    /// Anchors stay fixed for the lifetime of the layout, even after retirement.
    pub(crate) fn planned(events: &[Event]) -> Self {
        let paths: BTreeSet<_> = events
            .iter()
            .flat_map(|event| &event.changes)
            .flat_map(|change| {
                std::iter::once(change.path.as_str()).chain(match &change.kind {
                    Kind::Moved { from } => Some(from.as_str()),
                    _ => None,
                })
            })
            .collect();
        let mut layout = Self::default();
        let mut capacity = vec![0_usize];
        for path in paths {
            let index = layout.ensure_dir(parent_path(path));
            capacity.resize(layout.dirs.len(), 0);
            capacity[index] += 1;
        }
        layout.place_regions(&capacity);
        for (index, dir) in layout.dirs.iter_mut().enumerate() {
            dir.live = index == 0;
            dir.children = 0;
        }
        layout.planned = true;
        layout
    }

    fn place_regions(&mut self, capacity: &[usize]) {
        let mut children = vec![Vec::new(); self.dirs.len()];
        for (index, dir) in self.dirs.iter().enumerate().skip(1) {
            children[dir.parent.expect("non-root directories have parents")]
                .push(index);
        }
        let mut sizes = vec![Vec2::ZERO; self.dirs.len()];
        let mut offsets = vec![Vec2::ZERO; self.dirs.len()];
        let mut centres = vec![Vec2::ZERO; self.dirs.len()];
        for index in (0..self.dirs.len()).rev() {
            let diameter =
                2.0 * (SPACING * slot_count(capacity[index]).sqrt() + 0.4) + GAP;
            let mut rectangles = vec![(index, Vec2::splat(diameter))];
            rectangles
                .extend(children[index].iter().map(|&child| (child, sizes[child])));
            rectangles.sort_by(|a, b| {
                b.1.y
                    .total_cmp(&a.1.y)
                    .then(b.1.x.total_cmp(&a.1.x))
                    .then(a.0.cmp(&b.0))
            });
            let area: f32 = rectangles.iter().map(|(_, size)| size.x * size.y).sum();
            let width = rectangles
                .iter()
                .fold(area.sqrt(), |width, (_, size)| width.max(size.x));
            let mut cursor = Vec2::ZERO;
            let mut row_height = 0.0_f32;
            let mut extent = Vec2::ZERO;
            for (item, size) in rectangles {
                if cursor.x > 0.0 && cursor.x + size.x > width {
                    cursor.y += row_height;
                    cursor.x = 0.0;
                    row_height = 0.0;
                }
                if item == index {
                    centres[index] = cursor + size * 0.5;
                } else {
                    offsets[item] = cursor;
                }
                extent = extent.max(cursor + size);
                cursor.x += size.x;
                row_height = row_height.max(size.y);
            }
            sizes[index] = extent;
        }
        let origin = centres[0];
        for index in 0..self.dirs.len() {
            if let Some(parent) = self.dirs[index].parent {
                let parent_offset = offsets[parent];
                offsets[index] += parent_offset;
            }
            self.dirs[index].position = offsets[index] + centres[index] - origin;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::FileChange;

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

    fn assert_disjoint(layout: &Layout) {
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
    fn reserves_growth_without_moving_existing_directories() {
        let events = [
            event(&[("left/a", Kind::Added), ("right/a", Kind::Added)]),
            event(&[
                ("left/b", Kind::Added),
                ("left/deep/a", Kind::Added),
                ("right/deep/a", Kind::Added),
            ]),
        ];
        let mut layout = Layout::planned(&events);
        let positions: Vec<_> = layout.dirs.iter().map(|dir| dir.position).collect();
        for event in events {
            for change in event.changes {
                layout.add(&change.path, 0.0, 0);
            }
            for _ in 0..60 {
                layout.step(1.0 / 60.0);
                assert_disjoint(&layout);
                assert_eq!(
                    layout
                        .dirs
                        .iter()
                        .map(|dir| dir.position)
                        .collect::<Vec<_>>(),
                    positions
                );
            }
        }
    }

    #[test]
    fn deletion_rename_and_recreation_keep_reserved_anchors() {
        let events = [
            event(&[("a/file", Kind::Added), ("b/file", Kind::Added)]),
            event(&[("a/file", Kind::Deleted), ("b/file", Kind::Deleted)]),
            event(&[(
                "b/new",
                Kind::Moved {
                    from: "a/file".to_owned(),
                },
            )]),
        ];
        let mut layout = Layout::planned(&events);
        layout.add("a/file", 0.0, 0);
        layout.add("b/file", 0.0, 0);
        layout.settle(60, 1.0 / 60.0);
        let a = layout.dir_index["a"];
        let anchor = layout.dir(a).position;
        layout.remove("a/file", 0.6, 0);
        layout.remove("b/file", 0.6, 0);
        layout.rename("a/file", "b/new", 0);
        assert_disjoint(&layout);
        layout.settle(120, 1.0 / 60.0);
        assert!(!layout.dir(a).live);
        layout.add("a/file", 0.0, 0);
        assert_eq!(layout.dir(a).position, anchor);
        assert_disjoint(&layout);
    }

    #[test]
    fn planning_is_independent_of_change_order() {
        let mut events = vec![event(&[
            ("a/deep/file", Kind::Modified),
            ("b/file", Kind::Deleted),
            ("root", Kind::Added),
        ])];
        let first = Layout::planned(&events);
        events[0].changes.reverse();
        let second = Layout::planned(&events);
        assert_eq!(
            first
                .dirs
                .iter()
                .map(|dir| (&dir.path, dir.position))
                .collect::<Vec<_>>(),
            second
                .dirs
                .iter()
                .map(|dir| (&dir.path, dir.position))
                .collect::<Vec<_>>()
        );
        assert_eq!(first.dirs().count(), 1);
        assert_eq!(Layout::planned(&[]).dirs().count(), 1);
    }
}
