use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use bevy::math::Vec2;

use super::{Layout, MAX_ACCELERATION, MAX_SPEED, Presence, parent_path};
use crate::{model, timeline::Event};

const SAMPLE_TICKS: usize = 15;
const GAP: f32 = 0.3;
const MIN_RADIUS_FRACTION: f32 = 0.55;
pub(super) const RELEASE_RADIUS: f32 = 0.05;

#[derive(Clone, Debug, Default)]
pub(super) struct Plan {
    samples: Arc<[Vec<Vec2>]>,
    footprints: Arc<[Vec<Vec2>]>,
    pub(super) time: f32,
    origin: f32,
    step: f32,
}

#[derive(Clone, Copy, Debug)]
enum Region {
    Directory(usize),
    Split {
        left: usize,
        right: usize,
        axis: usize,
    },
}

impl Plan {
    fn new(layout: &mut Layout, events: &[Event], starts: &[f32]) -> Self {
        let started = Instant::now();
        for event in events {
            for change in &event.changes {
                layout.ensure_dir(parent_path(&change.path));
            }
        }
        let (required, step) = occupancy(layout, events, starts);
        let peak: Vec<_> = required
            .iter()
            .map(|row| row.iter().copied().fold(0.0, f32::max))
            .collect();
        let mut regions = Vec::new();
        let mut sizes = Vec::new();
        let mut children = vec![Vec::new(); layout.dir_count()];
        for (index, dir) in layout.dirs.iter().enumerate() {
            if let Some(parent) = dir.parent {
                children[parent].push(index);
            }
        }
        let root = subtree(&children, 0, &peak, &mut regions, &mut sizes);
        let mut window = 8;
        let samples = loop {
            // Dilation covers both box filters and the cubic spline's positive support.
            let reserved: Vec<_> = required
                .iter()
                .zip(&peak)
                .map(|(row, &peak)| {
                    maximum(row, window * 2 + 2)
                        .into_iter()
                        .map(|radius| radius.max(peak * MIN_RADIUS_FRACTION))
                        .collect()
                })
                .collect();
            let mut samples = pack(&regions, root, &reserved);
            anchor_motion(&mut samples, &required);
            for row in &mut samples {
                *row = average(&average(row, window), window);
            }
            let largest = samples
                .iter()
                .flat_map(|row| row.windows(2))
                .map(|pair| pair[0].distance(pair[1]))
                .fold(0.0, f32::max);
            let curvature = samples
                .iter()
                .flat_map(|row| row.windows(3))
                .map(|points| (points[0] - 2.0 * points[1] + points[2]).length())
                .fold(0.0, f32::max);
            if largest / step <= MAX_SPEED
                && curvature / (step * step) <= MAX_ACCELERATION
            {
                break samples;
            }
            window *= 2;
        };
        let footprints = required
            .iter()
            .map(|row| {
                let values: Vec<_> = maximum(row, 6)
                    .into_iter()
                    .map(|radius| Vec2::new(radius, f32::from(u8::from(radius > 0.0))))
                    .collect();
                average(&average(&values, 2), 2)
            })
            .collect::<Vec<_>>();
        eprintln!(
            "region_plan_seconds={:.6} directories={} samples={} easing_window_seconds={:.2}",
            started.elapsed().as_secs_f64(),
            samples.len(),
            samples.first().map_or(0, Vec::len),
            model::count_u32(u32::try_from(window).expect("smoothing window fits u32"))
                * step
        );
        Self {
            samples: samples.into(),
            footprints: footprints.into(),
            time: 0.0,
            origin: 0.0,
            step,
        }
    }

    pub(super) fn position(&self, dir: usize) -> Option<Vec2> {
        let row = self.samples.get(dir)?;
        Some(interpolate(row, self.time, self.step))
    }
}

fn interpolate(row: &[Vec2], time: f32, step: f32) -> Vec2 {
    let at = (time.max(0.0) / step).floor();
    let index = usize::try_from(std::time::Duration::from_secs_f32(at).as_secs())
        .unwrap_or(usize::MAX)
        .min(row.len() - 1);
    let fraction = ((time
        - model::count_u32(u32::try_from(index).expect("sample index fits u32"))
            * step)
        / step)
        .clamp(0.0, 1.0);
    let u = fraction;
    let weights = [
        (1.0 - u).powi(3),
        3.0 * u.powi(3) - 6.0 * u * u + 4.0,
        -3.0 * u.powi(3) + 3.0 * u * u + 3.0 * u + 1.0,
        u.powi(3),
    ];
    let controls = [
        index.saturating_sub(1),
        index,
        (index + 1).min(row.len() - 1),
        (index + 2).min(row.len() - 1),
    ];
    controls
        .into_iter()
        .zip(weights)
        .map(|(index, weight)| row[index] * (weight / 6.0))
        .sum()
}

fn occupancy(
    layout: &Layout,
    events: &[Event],
    starts: &[f32],
) -> (Vec<Vec<f32>>, f32) {
    let mut shadow = layout.clone();
    let duration = starts.last().copied().unwrap_or(0.5) + 2.5;
    let ticks = tick_at(duration);
    let sample_ticks = (usize::try_from(ticks).expect("tick count fits usize") / 8_192
        + 1)
    .max(SAMPLE_TICKS);
    let step = model::tick_time(
        u64::try_from(sample_ticks).expect("sample interval fits u64"),
    );
    let samples =
        usize::try_from(ticks).expect("tick count fits usize") / sample_ticks + 2;
    let mut required = vec![vec![0.0_f32; samples]; layout.dir_count()];
    let mut event_index = 0;
    let mut tick = 0;
    while tick <= ticks {
        let time = model::tick_time(tick);
        while starts.get(event_index).is_some_and(|&start| start <= time) {
            for change in &events[event_index].changes {
                model::apply(&mut shadow, change, 0);
            }
            event_index += 1;
        }
        shadow.fade_files(1.0 / 60.0);
        shadow.compact_slots();
        let mut active: Vec<_> = shadow.dirs.iter().map(|dir| dir.files > 0).collect();
        for index in (1..active.len()).rev() {
            if active[index] {
                active[shadow.dirs[index]
                    .parent
                    .expect("non-root directories have parents")] = true;
            }
        }
        active[0] = true;
        let sample =
            usize::try_from(tick).expect("tick count fits usize") / sample_ticks;
        for (index, dir) in shadow.dirs.iter().enumerate() {
            let radius = if active[index] { dir.radius() } else { 0.0 };
            required[index][sample] = required[index][sample].max(radius);
            required[index][sample + 1] = required[index][sample + 1].max(radius);
        }
        let settled = shadow
            .files
            .values()
            .all(|file| file.state == Presence::Present);
        if settled {
            let next_sample = u64::try_from((sample + 1) * sample_ticks)
                .expect("sample tick fits u64");
            let next_event = starts
                .get(event_index)
                .map_or(ticks.saturating_add(1), |&start| tick_at(start));
            tick = next_sample.min(next_event).max(tick + 1);
        } else {
            tick += 1;
        }
    }
    (required, step)
}

fn tick_at(time: f32) -> u64 {
    u64::try_from(
        (Duration::from_secs_f32(time).as_nanos() * u128::from(model::TICK_RATE))
            .div_ceil(1_000_000_000),
    )
    .expect("playback tick fits u64")
}

fn maximum(values: &[f32], window: usize) -> Vec<f32> {
    let mut deque: VecDeque<usize> = VecDeque::new();
    let mut end = 0;
    values
        .iter()
        .enumerate()
        .map(|(index, _)| {
            while end < values.len().min(index + window + 1) {
                while deque
                    .back()
                    .is_some_and(|&last| values[last] <= values[end])
                {
                    deque.pop_back();
                }
                deque.push_back(end);
                end += 1;
            }
            while deque
                .front()
                .is_some_and(|&first| first < index.saturating_sub(window))
            {
                deque.pop_front();
            }
            values[*deque.front().expect("window contains its centre")]
        })
        .collect()
}

fn average(values: &[Vec2], window: usize) -> Vec<Vec2> {
    let width = u16::try_from(window * 2 + 1).expect("smoothing window fits u16");
    let mut sum =
        values[0] * f32::from(u16::try_from(window + 1).expect("window fits u16"));
    for index in 1..=window {
        sum += values[index.min(values.len() - 1)];
    }
    let mut result = Vec::with_capacity(values.len());
    for index in 0..values.len() {
        result.push(sum / f32::from(width));
        sum -= values[index.saturating_sub(window)];
        sum += values[(index + window + 1).min(values.len() - 1)];
    }
    result
}

fn anchor_motion(samples: &mut [Vec<Vec2>], required: &[Vec<f32>]) {
    let mut previous: Vec<_> = samples.iter().map(|row| row[0]).collect();
    let mut shift = Vec2::ZERO;
    for sample in 1..samples[0].len() {
        let deltas: Vec<_> = samples
            .iter()
            .zip(&previous)
            .zip(required)
            .map(|((row, &previous), radii)| {
                let radius = radii[sample].min(radii[sample - 1]);
                let weight = if radius > 0.0 {
                    1.0 + ((radius - 0.4) / super::SPACING).powi(2)
                } else {
                    0.0
                };
                (row[sample] - previous, weight)
            })
            .collect();
        let total: f32 = deltas.iter().map(|&(_, weight)| weight).sum();
        let mut translation = if total > 0.0 {
            deltas
                .iter()
                .map(|&(delta, weight)| delta * weight)
                .sum::<Vec2>()
                / total
        } else {
            Vec2::ZERO
        };
        for _ in 0..16 {
            let mut sum = Vec2::ZERO;
            let mut total = 0.0;
            for &(delta, weight) in &deltas {
                let influence = weight / delta.distance(translation).max(0.0001);
                sum += delta * influence;
                total += influence;
            }
            if total > 0.0 {
                translation = sum / total;
            }
        }
        shift += translation;
        for (row, previous) in samples.iter_mut().zip(&mut previous) {
            *previous = row[sample];
            row[sample] -= shift;
        }
    }
}

fn subtree(
    children: &[Vec<usize>],
    dir: usize,
    peak: &[f32],
    regions: &mut Vec<Region>,
    sizes: &mut Vec<Vec2>,
) -> usize {
    let own = regions.len();
    regions.push(Region::Directory(dir));
    sizes.push(Vec2::splat(2.0 * peak[dir] + GAP));
    let mut entries = vec![own];
    for &child in &children[dir] {
        entries.push(subtree(children, child, peak, regions, sizes));
    }
    combine(&entries, regions, sizes)
}

fn combine(
    entries: &[usize],
    regions: &mut Vec<Region>,
    sizes: &mut Vec<Vec2>,
) -> usize {
    if entries.len() == 1 {
        return entries[0];
    }
    let total: f32 = entries
        .iter()
        .map(|&entry| sizes[entry].element_product())
        .sum();
    let mut accumulated = 0.0;
    let mut middle = 1;
    let mut imbalance = f32::INFINITY;
    for (offset, &entry) in entries[..entries.len() - 1].iter().enumerate() {
        accumulated += sizes[entry].element_product();
        let difference = (total * 0.5 - accumulated).abs();
        if difference < imbalance {
            imbalance = difference;
            middle = offset + 1;
        }
    }
    let left = combine(&entries[..middle], regions, sizes);
    let right = combine(&entries[middle..], regions, sizes);
    let a = sizes[left];
    let b = sizes[right];
    let horizontal = Vec2::new(a.x + b.x, a.y.max(b.y));
    let vertical = Vec2::new(a.x.max(b.x), a.y + b.y);
    let aspect = |size: Vec2| (size.x / size.y).ln().abs();
    let axis = usize::from(aspect(vertical) < aspect(horizontal));
    let index = regions.len();
    regions.push(Region::Split { left, right, axis });
    sizes.push(if axis == 0 { horizontal } else { vertical });
    index
}

fn pack(regions: &[Region], root: usize, reserved: &[Vec<f32>]) -> Vec<Vec<Vec2>> {
    let samples = reserved[0].len();
    let mut result = vec![vec![Vec2::ZERO; samples]; reserved.len()];
    let mut sizes = vec![Vec2::ZERO; regions.len()];
    let mut origins = vec![Vec2::ZERO; regions.len()];
    for sample in 0..samples {
        for (index, region) in regions.iter().enumerate() {
            sizes[index] = match *region {
                Region::Directory(dir) => {
                    Vec2::splat(2.0 * reserved[dir][sample] + GAP)
                }
                Region::Split { left, right, axis } => {
                    let mut size = sizes[left].max(sizes[right]);
                    size[axis] = sizes[left][axis] + sizes[right][axis];
                    size
                }
            };
        }
        origins[root] = Vec2::ZERO;
        for (index, region) in regions.iter().enumerate().rev() {
            match *region {
                Region::Directory(dir) => {
                    result[dir][sample] = origins[index] + sizes[index] * 0.5;
                }
                Region::Split { left, right, axis } => {
                    origins[left] = origins[index];
                    origins[right] = origins[index];
                    origins[right][axis] += sizes[left][axis];
                    let other = 1 - axis;
                    origins[left][other] +=
                        (sizes[index][other] - sizes[left][other]) * 0.5;
                    origins[right][other] +=
                        (sizes[index][other] - sizes[right][other]) * 0.5;
                }
            }
        }
        let anchor = result[0][sample];
        for row in &mut result {
            row[sample] -= anchor;
        }
    }
    result
}

impl Layout {
    pub(crate) fn forecast(&self) -> Vec<(Vec2, f32)> {
        let mut centres = vec![Vec2::ZERO; self.dirs.len()];
        let mut points = Vec::new();
        for (index, row) in self.plan.footprints.iter().enumerate() {
            let footprint = interpolate(row, self.plan.time, self.plan.step);
            let dir = &self.dirs[index];
            let centre = dir.parent.map_or(dir.position, |parent| {
                centres[parent].lerp(dir.position, footprint.y.clamp(0.0, 1.0))
            });
            centres[index] = centre;
            if footprint.x > 0.0001 {
                points.push((centre, footprint.x));
            }
        }
        points
    }

    #[cfg(test)]
    pub(crate) fn planned(events: &[Event]) -> Self {
        let starts = model::event_starts(events, 0.5, 1.0);
        Self::scheduled(events, &starts)
    }

    pub(crate) fn scheduled(events: &[Event], starts: &[f32]) -> Self {
        let mut layout = Self::empty();
        if !events.is_empty() {
            layout.plan = Plan::new(&mut layout, events, starts);
        }
        for (index, dir) in layout.dirs.iter_mut().enumerate() {
            dir.live = index == 0;
            dir.activated = index == 0;
            dir.children = 0;
            if let Some(position) = layout.plan.position(index) {
                dir.position = position;
            }
        }
        layout
    }

    pub(crate) fn plan_time(&mut self, time: f32) {
        self.plan.time = time;
        self.plan.origin = time;
    }

    pub(crate) fn sample_plan(&mut self, time: f32, dt: f32) {
        self.plan.time = self.plan.origin + time - dt;
    }

    pub(super) fn relax(&mut self, dt: f32) {
        self.plan.time += dt;
        if self.plan.samples.is_empty() {
            self.relax_unplanned(dt);
            return;
        }
        for (index, dir) in self.dirs.iter_mut().enumerate() {
            dir.position = self
                .plan
                .position(index)
                .expect("planned directories have trajectories");
            let target = if index != 0 && dir.files == 0 && dir.children == 0 {
                0.0
            } else {
                dir.radius()
            };
            dir.reserve += (target - dir.reserve) * (1.0 - (-dt / 0.2).exp());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contributors::Mappings,
        model::{Options, Simulation},
        theme::Theme,
        timeline::{FileChange, Identity, Kind},
    };

    fn event(seconds: i64, changes: &[(&str, Kind)]) -> Event {
        Event {
            seconds,
            author: Identity {
                name: b"Ada".to_vec(),
                email: Vec::new(),
            },
            contributor: None,
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

    fn burst() -> Vec<Event> {
        let mut events = vec![event(0, &[("z/keep", Kind::Added)])];
        for day in 1..20 {
            events.push(event(day * 86_400, &[("a/keep", Kind::Modified)]));
        }
        let mut growth = event(20 * 86_400, &[]);
        growth.changes = (0..80)
            .map(|index| FileChange {
                path: format!("a/deep/{index}"),
                kind: Kind::Added,
                insertions: 1,
                removals: 0,
            })
            .collect();
        events.push(growth);
        events.push(event(
            21 * 86_400,
            &[(
                "z/moved",
                Kind::Moved {
                    from: "a/keep".to_owned(),
                },
            )],
        ));
        let mut deletion = event(22 * 86_400, &[]);
        deletion.changes = (0..79)
            .map(|index| FileChange {
                path: format!("a/deep/{index}"),
                kind: Kind::Deleted,
                insertions: 0,
                removals: 1,
            })
            .collect();
        events.push(deletion);
        events
    }

    #[test]
    fn global_sibling_order_never_changes_even_when_later_paths_sort_first() {
        let events = burst();
        let layout = Layout::planned(&events);
        let samples = &layout.plan.samples;
        for a in 0..samples.len() {
            for b in a + 1..samples.len() {
                let mut low = Vec2::splat(f32::INFINITY);
                let mut high = Vec2::splat(f32::NEG_INFINITY);
                for (&a, &b) in samples[a].iter().zip(&samples[b]) {
                    low = low.min(b - a);
                    high = high.max(b - a);
                }
                assert!(low.x > 0.0 || high.x < 0.0 || low.y > 0.0 || high.y < 0.0);
            }
        }
        let mut replay = layout.clone();
        replay.step(1.0 / 60.0);
        assert!(Arc::ptr_eq(&layout.plan.samples, &replay.plan.samples));
        assert_eq!(layout.plan.time, 0.0);
    }

    #[test]
    fn region_space_opens_before_a_subtree_burst_without_lifetime_reservation() {
        let events = burst();
        let layout = Layout::planned(&events);
        let a = layout.dir_index["a/deep"];
        let z = layout.dir_index["a"];
        let mut plan = layout.plan.clone();
        let separation = |plan: &Plan| {
            plan.position(a)
                .unwrap()
                .distance(plan.position(z).unwrap())
        };
        let early = separation(&plan);
        plan.time = 10.0;
        let before = separation(&plan);
        plan.time = 10.5;
        let at_growth = separation(&plan);
        assert!(before > early + 0.5, "early={early}, before={before}");
        assert!(at_growth >= before);
        let mut sim = Simulation::new(
            events,
            &Options::default(),
            &Theme::neon(),
            &Mappings::default(),
        );
        for tick in 0..850 {
            sim.advance_to(tick, 60);
            let dirs: Vec<_> = sim.layout().dirs().collect();
            for (offset, (_, a)) in dirs.iter().enumerate() {
                for (_, b) in &dirs[offset + 1..] {
                    assert!(
                        a.position.distance(b.position) + 0.0001
                            >= a.radius() + b.radius(),
                        "{} overlaps {} at {tick}",
                        a.path,
                        b.path
                    );
                }
            }
        }
    }

    #[test]
    fn retained_files_and_directories_have_bounded_tick_velocity_and_acceleration() {
        use std::collections::BTreeMap;
        let mut sim = Simulation::new(
            burst(),
            &Options::default(),
            &Theme::neon(),
            &Mappings::default(),
        );
        let dt = 1.0 / 60.0;
        let mut previous = BTreeMap::new();
        let mut velocities = BTreeMap::new();
        for tick in 0..950 {
            sim.advance_to(tick, 60);
            let positions: BTreeMap<_, _> = sim
                .layout()
                .files()
                .map(|(id, file)| ((false, id), file.position))
                .chain(sim.layout().dirs().map(|(id, dir)| {
                    ((true, u64::try_from(id).unwrap()), dir.position)
                }))
                .collect();
            let mut next_velocities = BTreeMap::new();
            for (&id, &position) in &positions {
                let Some(&before) = previous.get(&id) else {
                    continue;
                };
                let velocity: Vec2 = (position - before) / dt;
                assert!(
                    velocity.length() <= MAX_SPEED + 0.001,
                    "speed {:?}: {} at {tick}",
                    id,
                    velocity.length()
                );
                if let Some(&before) = velocities.get(&id) {
                    let acceleration: Vec2 = (velocity - before) / dt;
                    assert!(
                        acceleration.length() <= MAX_ACCELERATION + 0.02,
                        "acceleration {:?}: {} at {tick}",
                        id,
                        acceleration.length()
                    );
                }
                next_velocities.insert(id, velocity);
            }
            previous = positions;
            velocities = next_velocities;
        }
    }

    #[test]
    fn camera_opens_before_growth_without_a_burst_tick_zoom() {
        let mut sim = Simulation::new(
            burst(),
            &Options::default(),
            &Theme::neon(),
            &Mappings::default(),
        );
        let mut previous = sim.view().1;
        let mut early = 0.0;
        let mut before = 0.0;
        for tick in 0..700 {
            sim.advance_to(tick, 60);
            let distance = sim.view().1;
            if tick == 300 {
                early = distance;
            }
            if tick == 600 {
                before = distance;
            }
            if tick >= 570 {
                assert!(
                    (distance - previous).abs() / previous < 0.02,
                    "camera zoom at {tick}: {previous} -> {distance}"
                );
            }
            previous = distance;
        }
        assert!(before > early);
    }

    #[test]
    fn maximum_matches_a_brute_force_window() {
        let values = [0.0_f32, 1.0, 0.0, 4.0, 2.0, 0.0];
        for window in [0, 1, 4, 20] {
            let expected: Vec<_> = (0..values.len())
                .map(|index| {
                    values[index.saturating_sub(window)
                        ..values.len().min(index + window + 1)]
                        .iter()
                        .copied()
                        .fold(0.0, f32::max)
                })
                .collect();
            assert_eq!(maximum(&values, window), expected);
        }
    }

    #[test]
    fn long_idle_histories_skip_settled_ticks_and_bound_schedule_storage() {
        let events: Vec<_> = (0..1_000)
            .map(|day| event(i64::from(day) * 86_400, &[("a/file", Kind::Modified)]))
            .collect();
        let starts = model::event_starts(&events, 86_400.0, 86_400.0);
        assert!(tick_at(starts[999]) > u64::from(u32::MAX));
        let layout = Layout::scheduled(&events, &starts);
        assert!(layout.plan.samples[0].len() <= 8_194);
    }

    #[test]
    #[ignore = "developer planning cost measurement"]
    fn planning_cost_large_inline_history() {
        let events: Vec<_> = (0..1_000)
            .map(|commit| {
                let mut entry = event(i64::from(commit) * 3_600, &[]);
                entry.changes = (0..12)
                    .map(|file| {
                        let ordinal = commit * 12 + file;
                        FileChange {
                            path: format!(
                                "crate-{}/src/{}.rs",
                                ordinal / 4,
                                ordinal % 4
                            ),
                            kind: Kind::Added,
                            insertions: 1,
                            removals: 0,
                        }
                    })
                    .collect();
                entry
            })
            .collect();
        let started = Instant::now();
        let layout = Layout::planned(&events);
        eprintln!(
            "large_inline_plan_seconds={:.6} events={} directories={} files=12000",
            started.elapsed().as_secs_f64(),
            events.len(),
            layout.dir_count()
        );
        assert_eq!(layout.dir_count(), 6_001);
        assert!(layout.plan.samples[0].len() <= 8_194);
    }
}
