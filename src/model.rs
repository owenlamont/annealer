use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    time::Duration,
};

use bevy::math::{Vec2, Vec3};

use crate::{
    layout::Layout,
    theme::{Geometry, Theme},
    timeline::{Event, Kind},
};

pub(super) const TICK_RATE: u32 = 60;
pub(super) const PREVIEW_FPS: u16 = 30;
const LEAD_IN: f32 = 0.5;
const TAIL: f32 = 2.5;
const PULSE_SECONDS: f32 = 1.3;
const ADD_DELAY: f32 = 0.2;
const DELETE_DELAY: f32 = 0.6;
const MIN_AVATAR_HEIGHT: f32 = 3.2;
const AVATAR_CLEARANCE: f32 = 0.04;
const AVATAR_IDLE: f32 = 5.0;
const BEAM_LIMIT: usize = 24;
const FRAGMENT_TARGETS: usize = 24;
const SETTLE_TICKS: u32 = 1200;
const DIR_LABELS: usize = 20;

pub(super) struct Options {
    pub commit_seconds: f32,
    pub start: usize,
    pub commits: Option<usize>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            commit_seconds: 0.5,
            start: 0,
            commits: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Target {
    File(u64),
    Dir(usize),
}

#[derive(Clone, Debug)]
struct Pulse {
    start: f32,
    end: f32,
    author: usize,
    targets: Vec<(Target, f32, u32, u32)>,
    aggregated: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Avatar {
    pub name: String,
    pub position: Vec3,
    velocity: Vec3,
    target: Vec3,
    last_active: f32,
    pub presence: f32,
}

pub(super) struct Beam {
    pub author: usize,
    pub from: Vec3,
    pub to: Vec3,
    pub width: f32,
}

pub(super) struct Fragment {
    pub position: Vec3,
    pub rising: bool,
    pub scale: f32,
}

/// Placement priority, highest first, when labels would overlap on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum LabelKind {
    Avatar,
    Landmark,
    Activity,
}

pub(super) struct Label {
    pub position: Vec3,
    pub text: String,
    pub kind: LabelKind,
}

#[derive(Clone, Debug)]
struct State {
    tick: u64,
    layout: Layout,
    avatars: Vec<Avatar>,
    authors: HashMap<String, usize>,
    next_event: usize,
    pulses: VecDeque<Pulse>,
    focus: Vec3,
    distance: f32,
    geometry: Geometry,
}

/// Deterministic playback stepped at a fixed tick rate, independent of output frame rate.
pub(super) struct Simulation {
    events: Vec<Event>,
    offset: usize,
    total: usize,
    commit_seconds: f32,
    initial: State,
    state: State,
    theme: Theme,
}

impl Simulation {
    pub(super) fn new(events: &[Event], options: &Options, theme: &Theme) -> Self {
        let start = options.start.min(events.len());
        let end = options
            .commits
            .map_or(events.len(), |count| (start + count).min(events.len()));
        let mut state = State {
            tick: 0,
            layout: Layout::default(),
            avatars: Vec::new(),
            authors: HashMap::new(),
            next_event: 0,
            pulses: VecDeque::new(),
            focus: Vec3::ZERO,
            distance: 12.0,
            geometry: theme.geometry(),
        };
        for event in &events[..start] {
            let author = state.author(&event.author, 0.0);
            for change in &event.changes {
                apply(&mut state.layout, change, author);
            }
        }
        if start > 0 {
            state.layout.settle(SETTLE_TICKS, tick_seconds());
            state.avatars.clear();
            state.authors.clear();
        }
        (state.focus, state.distance) = state.fit();
        Self {
            events: events[start..end].to_vec(),
            offset: start,
            total: events.len(),
            commit_seconds: options.commit_seconds,
            initial: state.clone(),
            state,
            theme: theme.clone(),
        }
    }

    pub(super) fn duration(&self) -> f32 {
        LEAD_IN + self.commit_seconds * count(self.events.len()) + TAIL
    }

    pub(super) fn seconds(&self) -> f32 {
        tick_time(self.state.tick)
    }

    pub(super) fn advance_to(&mut self, frame: u32, fps: u16) {
        let target = u64::from(frame) * u64::from(TICK_RATE) / u64::from(fps);
        if target < self.state.tick {
            self.state = self.initial.clone();
        }
        while self.state.tick < target {
            self.tick();
        }
    }

    fn tick(&mut self) {
        let state = &mut self.state;
        state.tick += 1;
        let now = tick_time(state.tick);
        while let Some(event) = self.events.get(state.next_event) {
            let start = LEAD_IN + self.commit_seconds * count(state.next_event);
            if start > now {
                break;
            }
            state.begin(event, start);
            state.next_event += 1;
        }
        let dt = tick_seconds();
        state.layout.step(dt);
        state.pulses.retain(|pulse| now < pulse.end);
        for avatar in &mut state.avatars {
            let omega = 3.0;
            let acceleration = (avatar.target - avatar.position) * omega * omega
                - avatar.velocity * 2.0 * omega;
            avatar.velocity += acceleration * dt;
            avatar.velocity.y = 0.0;
            avatar.position += avatar.velocity * dt;
            let visible = now - avatar.last_active < AVATAR_IDLE;
            let fade = if visible { dt / 0.4 } else { -dt / 1.2 };
            avatar.presence = (avatar.presence + fade).clamp(0.0, 1.0);
        }
        state.place_view(dt);
    }

    pub(super) fn theme(&self) -> &Theme {
        &self.theme
    }

    pub(super) fn layout(&self) -> &Layout {
        &self.state.layout
    }

    pub(super) fn avatars(&self) -> &[Avatar] {
        &self.state.avatars
    }

    /// Smoothed camera focus and distance along [`view_direction`].
    pub(super) fn view(&self) -> (Vec3, f32) {
        (self.state.focus, self.state.distance)
    }

    pub(super) fn scale(&self) -> f32 {
        self.state.scale()
    }

    pub(super) fn commit(&self) -> usize {
        self.offset + self.state.next_event
    }

    pub(super) fn total(&self) -> usize {
        self.total
    }

    pub(super) fn caption(&self) -> String {
        let Some(event) = self
            .state
            .next_event
            .checked_sub(1)
            .and_then(|index| self.events.get(index))
        else {
            return format!("{} / {} commits", self.commit(), self.total());
        };
        format!(
            "{}   |   {}   |   commit {} / {}",
            date(event.seconds),
            event.author,
            self.commit(),
            self.total
        )
    }

    pub(super) fn beams(&self) -> Vec<Beam> {
        let now = self.seconds();
        let mut beams = Vec::new();
        for pulse in &self.state.pulses {
            let envelope = envelope(now - pulse.start, pulse.end - pulse.start);
            if envelope <= 0.0 {
                continue;
            }
            let from = self.state.avatars[pulse.author].position;
            for &(target, weight, ..) in &pulse.targets {
                if let Some(to) = self.state.point(target) {
                    beams.push(Beam {
                        author: pulse.author,
                        from,
                        to: to + Vec3::Y * 0.12,
                        width: envelope * (1.0 + weight.log2() * 0.35).min(3.0),
                    });
                }
            }
        }
        beams
    }

    pub(super) fn fragments(&self) -> Vec<Fragment> {
        let now = self.seconds();
        let mut fragments = Vec::new();
        for pulse in &self.state.pulses {
            let age = now - pulse.start;
            let envelope = envelope(age, pulse.end - pulse.start);
            if envelope <= 0.0 {
                continue;
            }
            let from = self.state.avatars[pulse.author].position;
            for &(target, _, insertions, removals) in
                pulse.targets.iter().take(FRAGMENT_TARGETS)
            {
                let Some(to) = self.state.point(target) else {
                    continue;
                };
                fragments
                    .extend(diff_fragments(age, envelope, from, to, insertions, false));
                fragments
                    .extend(diff_fragments(age, envelope, from, to, removals, true));
            }
        }
        fragments
    }

    pub(super) fn labels(&self) -> Vec<Label> {
        let layout = &self.state.layout;
        let mut subtree = vec![0_usize; layout.dir_count()];
        let mut centroids = vec![Vec2::ZERO; layout.dir_count()];
        for (index, dir) in layout.dirs() {
            let mut current = Some(index);
            while let Some(ancestor) = current {
                subtree[ancestor] += dir.files;
                centroids[ancestor] += dir.position * count(dir.files);
                current = layout.dir(ancestor).parent;
            }
        }
        let mut dirs: Vec<_> = layout.dirs().filter(|(_, dir)| dir.depth > 0).collect();
        dirs.sort_by_key(|&(index, dir)| {
            (
                std::cmp::Reverse(subtree[index] >> dir.depth.min(16)),
                dir.depth,
                index,
            )
        });
        let mut labels: Vec<Label> = dirs
            .into_iter()
            .take(DIR_LABELS)
            .map(|(index, dir)| Label {
                position: ground(centroids[index] / count(subtree[index].max(1))),
                text: format!("{}/", dir.name()),
                kind: LabelKind::Landmark,
            })
            .collect();
        for pulse in &self.state.pulses {
            if pulse.targets.len() > 8 && !pulse.aggregated {
                continue;
            }
            for &(target, ..) in &pulse.targets {
                let text = match target {
                    Target::File(id) => {
                        layout.file(id).map(|file| file.name().to_owned())
                    }
                    Target::Dir(index) => Some(format!("{}/", layout.dir(index).path)),
                };
                if let (Some(text), Some(position)) = (text, self.state.point(target)) {
                    labels.push(Label {
                        position: position + Vec3::new(-0.2, 0.0, 0.45),
                        text,
                        kind: LabelKind::Activity,
                    });
                }
            }
        }
        for avatar in &self.state.avatars {
            if avatar.presence > 0.05 {
                labels.push(Label {
                    position: avatar.position + self.state.geometry.avatar_label_offset,
                    text: avatar.name.clone(),
                    kind: LabelKind::Avatar,
                });
            }
        }
        labels
    }
}

impl State {
    fn author(&mut self, name: &str, now: f32) -> usize {
        if let Some(&index) = self.authors.get(name) {
            return index;
        }
        let index = self.avatars.len();
        self.avatars.push(Avatar {
            name: name.to_owned(),
            position: self.focus() + Vec3::Y * MIN_AVATAR_HEIGHT,
            velocity: Vec3::ZERO,
            target: self.focus(),
            last_active: now,
            presence: 0.0,
        });
        self.authors.insert(name.to_owned(), index);
        index
    }

    fn begin(&mut self, event: &Event, start: f32) {
        let author = self.author(&event.author, start);
        let mut touched = BTreeMap::new();
        for change in &event.changes {
            let id = apply(&mut self.layout, change, author);
            touched.insert(id, (change.insertions, change.removals));
        }
        let mut targets: Vec<_> = touched
            .iter()
            .map(|(&id, &(insertions, removals))| {
                (Target::File(id), 1.0, insertions, removals)
            })
            .collect();
        let aggregated = targets.len() > BEAM_LIMIT;
        if aggregated {
            targets = self.aggregate(&touched);
        }
        let centroid = targets
            .iter()
            .filter_map(|&(target, ..)| self.destination(target))
            .fold((Vec2::ZERO, 0.0), |(sum, n), point| (sum + point, n + 1.0));
        for pulse in &mut self.pulses {
            if pulse.author == author {
                pulse.end = pulse.end.min(start + 0.3);
            }
        }
        let avatar = &mut self.avatars[author];
        if centroid.1 > 0.0 {
            avatar.target = ground(centroid.0 / centroid.1);
        }
        if avatar.presence <= 0.0 {
            avatar.position = avatar.target + Vec3::Y * MIN_AVATAR_HEIGHT;
            avatar.velocity = Vec3::ZERO;
        }
        avatar.last_active = start;
        self.pulses.push_back(Pulse {
            start,
            end: start + PULSE_SECONDS,
            author,
            targets,
            aggregated,
        });
    }

    fn aggregate(
        &self,
        touched: &BTreeMap<u64, (u32, u32)>,
    ) -> Vec<(Target, f32, u32, u32)> {
        let mut groups: BTreeMap<usize, (f32, u32, u32)> = BTreeMap::new();
        for (&id, &(insertions, removals)) in touched {
            if let Some(file) = self.layout.file(id) {
                let entry = groups.entry(file.dir).or_default();
                *entry = (entry.0 + 1.0, entry.1 + insertions, entry.2 + removals);
            }
        }
        while groups.len() > BEAM_LIMIT {
            let deepest = groups
                .keys()
                .map(|&dir| self.layout.dir(dir).depth)
                .max()
                .unwrap_or(0);
            if deepest == 0 {
                break;
            }
            let mut lifted = BTreeMap::new();
            for (dir, (weight, insertions, removals)) in groups {
                let entry = self.layout.dir(dir);
                let key = if entry.depth == deepest {
                    entry.parent.unwrap_or(dir)
                } else {
                    dir
                };
                let total: &mut (f32, u32, u32) = lifted.entry(key).or_default();
                *total = (total.0 + weight, total.1 + insertions, total.2 + removals);
            }
            groups = lifted;
        }
        groups
            .into_iter()
            .map(|(dir, (weight, insertions, removals))| {
                (Target::Dir(dir), weight, insertions, removals)
            })
            .collect()
    }

    fn destination(&self, target: Target) -> Option<Vec2> {
        match target {
            Target::File(id) => self.layout.target(id),
            Target::Dir(index) => Some(self.layout.dir(index).position),
        }
    }

    fn point(&self, target: Target) -> Option<Vec3> {
        match target {
            Target::File(id) => self.layout.file(id).map(|file| ground(file.position)),
            Target::Dir(index) => Some(ground(self.layout.dir(index).position)),
        }
    }

    fn focus(&self) -> Vec3 {
        self.focus
    }

    fn scale(&self) -> f32 {
        1.0 + self.distance / 60.0
    }

    fn file_tops(&self) -> Vec<Vec3> {
        self.layout
            .files()
            .flat_map(|(_, file)| {
                let low = self.geometry.file_min * file.presence.max(0.001);
                let high = self.geometry.file_max * file.presence.max(0.001);
                [
                    Vec3::new(low.x, high.y, low.z),
                    Vec3::new(low.x, high.y, high.z),
                    Vec3::new(high.x, high.y, low.z),
                    high,
                ]
                .map(|corner| ground(file.position) + corner)
            })
            .collect()
    }

    fn scene_points(&self, tops: &[Vec3]) -> Vec<Vec3> {
        let mut points: Vec<Vec3> = self
            .layout
            .dirs()
            .flat_map(|(_, dir)| {
                let centre = ground(dir.position);
                let radius = dir.radius();
                [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
                    .map(|side| centre + side * radius)
            })
            .collect();
        points.extend_from_slice(tops);
        points
    }

    fn fit(&self) -> (Vec3, f32) {
        let (low, high) = self.layout.bounds();
        let mut focus = ground((low + high) * 0.5);
        let tops = self.file_tops();
        let points = self.scene_points(&tops);
        let direction = view_direction();
        let ground_up = Vec3::new(-direction.x, 0.0, -direction.z).normalize();
        let ground_right = ground_up.cross(Vec3::Y);
        for _ in 0..4 {
            let distance = self.fit_distance(focus, &points, &tops);
            let lifted = self.lifted_points(focus, distance, &tops);
            let all: Vec<_> = points.iter().chain(&lifted).copied().collect();
            let (low, high) = screen_bounds(focus + direction * distance, focus, &all);
            let centre = (low + high) * 0.5 - Vec2::new(0.0, -0.04);
            let span = distance * (std::f32::consts::FRAC_PI_8).tan();
            focus += ground_right * centre.x * span * 16.0 / 9.0
                + ground_up * centre.y * span * 1.2;
        }
        (focus, self.fit_distance(focus, &points, &tops))
    }

    fn fit_distance(&self, focus: Vec3, points: &[Vec3], tops: &[Vec3]) -> f32 {
        let direction = view_direction();
        let (mut near, mut far) = (2.0_f32, 20_000.0_f32);
        for _ in 0..32 {
            let middle = f32::midpoint(near, far);
            let mut all = self.lifted_points(focus, middle, tops);
            all.extend_from_slice(points);
            if in_view(focus + direction * middle, focus, &all) {
                far = middle;
            } else {
                near = middle;
            }
        }
        far
    }

    fn lifted_points(&self, focus: Vec3, distance: f32, tops: &[Vec3]) -> Vec<Vec3> {
        let eye = focus + view_direction() * distance;
        let line =
            screen_bounds(eye, focus, tops).1.y.max(-0.8) + 2.0 * AVATAR_CLEARANCE;
        let scale = 1.0 + distance / 60.0;
        self.avatars
            .iter()
            .filter(|avatar| avatar.presence > 0.0)
            .flat_map(|avatar| {
                let height = self.lift(avatar, focus, distance, line);
                let position = Vec3::new(avatar.position.x, height, avatar.position.z);
                let mut points = box_corners(
                    Vec3::splat(-self.geometry.avatar_radius * scale),
                    Vec3::splat(self.geometry.avatar_radius * scale),
                )
                .map(|corner| position + corner)
                .to_vec();
                let label = position + self.geometry.avatar_label_offset;
                points.push(label);
                points
            })
            .collect()
    }

    fn lift(&self, avatar: &Avatar, focus: Vec3, distance: f32, line: f32) -> f32 {
        let eye = focus + view_direction() * distance;
        let forward = -view_direction();
        let up = forward.cross(Vec3::Y).normalize().cross(forward);
        let radius = self.geometry.avatar_radius * (1.0 + distance / 60.0);
        let base = Vec3::new(avatar.position.x, 0.0, avatar.position.z);
        let solve = |offset: Vec3, target: f32| {
            let slope = target * std::f32::consts::FRAC_PI_8.tan();
            let delta = base + offset - eye;
            (slope * delta.dot(forward) - delta.dot(up)) / (up.y - slope * forward.y)
        };
        let height = box_corners(Vec3::splat(-radius), Vec3::splat(radius))
            .into_iter()
            .map(|corner| solve(corner, line))
            .fold(MIN_AVATAR_HEIGHT, f32::max)
            .max(solve(
                self.geometry.avatar_label_offset,
                line + 2.0 * self.geometry.label_height / 720.0,
            ));
        // Immediate upward correction preserves clearance while downward easing avoids bobbing.
        height.max(
            avatar.position.y
                + (height - avatar.position.y) * (1.0 - (-tick_seconds() / 1.5).exp()),
        ) + 0.0001
    }

    fn place_view(&mut self, dt: f32) {
        let (focus, distance) = self.fit();
        let settle = if distance > self.distance { 0.3 } else { 1.5 };
        let ease = 1.0 - (-dt / settle).exp();
        self.focus += (focus - self.focus) * ease;
        self.distance += (distance - self.distance) * ease;
        let tops = self.file_tops();
        let points = self.scene_points(&tops);
        self.distance = self
            .distance
            .max(self.fit_distance(self.focus, &points, &tops));
        let line = screen_bounds(
            self.focus + view_direction() * self.distance,
            self.focus,
            &tops,
        )
        .1
        .y
        .max(-0.8)
            + 2.0 * AVATAR_CLEARANCE;
        let heights: Vec<_> = self
            .avatars
            .iter()
            .map(|avatar| self.lift(avatar, self.focus, self.distance, line))
            .collect();
        for (avatar, height) in self.avatars.iter_mut().zip(heights) {
            avatar.position.y = height;
        }
    }
}

fn apply(
    layout: &mut Layout,
    change: &crate::timeline::FileChange,
    author: usize,
) -> u64 {
    match &change.kind {
        Kind::Added => layout.add(&change.path, ADD_DELAY, author),
        Kind::Deleted => layout
            .remove(&change.path, DELETE_DELAY, author)
            .unwrap_or_else(|| layout.touch(&change.path, author)),
        Kind::Modified => layout.touch(&change.path, author),
        Kind::Moved { from } => layout.rename(from, &change.path, author),
    }
}

pub(super) fn view_direction() -> Vec3 {
    Vec3::new(0.4, 0.44, 0.95).normalize()
}

fn box_corners(low: Vec3, high: Vec3) -> [Vec3; 8] {
    [
        low,
        Vec3::new(low.x, low.y, high.z),
        Vec3::new(low.x, high.y, low.z),
        Vec3::new(low.x, high.y, high.z),
        Vec3::new(high.x, low.y, low.z),
        Vec3::new(high.x, low.y, high.z),
        Vec3::new(high.x, high.y, low.z),
        high,
    ]
}

/// Projects points to normalised screen coordinates, or `None` for one behind the eye.
fn project(
    eye: Vec3,
    focus: Vec3,
    points: &[Vec3],
) -> impl Iterator<Item = Option<Vec2>> {
    let forward = (focus - eye).normalize();
    let right = forward.cross(Vec3::Y).normalize();
    let up = right.cross(forward);
    let half_height = (std::f32::consts::FRAC_PI_8).tan();
    points.iter().map(move |&point| {
        let offset = point - eye;
        let depth = offset.dot(forward);
        (depth > 0.5).then(|| {
            Vec2::new(
                offset.dot(right) / (depth * half_height * 16.0 / 9.0),
                offset.dot(up) / (depth * half_height),
            )
        })
    })
}

/// Whether every point projects inside the margins left by the title and caption.
fn in_view(eye: Vec3, focus: Vec3, points: &[Vec3]) -> bool {
    project(eye, focus, points).all(|point| {
        point.is_some_and(|point| {
            point.x.abs() <= 0.92 && (-0.8..=0.72).contains(&point.y)
        })
    })
}

fn screen_bounds(eye: Vec3, focus: Vec3, points: &[Vec3]) -> (Vec2, Vec2) {
    project(eye, focus, points).flatten().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(low, high), point| (low.min(point), high.max(point)),
    )
}

/// Up to three pieces per direction, descending along the beam or rising from the file.
fn diff_fragments(
    age: f32,
    envelope: f32,
    from: Vec3,
    to: Vec3,
    lines: u32,
    rising: bool,
) -> impl Iterator<Item = Fragment> {
    let pieces = lines.bit_width().min(3);
    (0..pieces).map(move |piece| {
        let phase = (age * 1.1 + count_u32(piece) / count_u32(pieces)).fract();
        let position = if rising {
            to + Vec3::new(0.15, 0.2 + phase * 2.2, -0.1)
        } else {
            from.lerp(to + Vec3::Y * 0.2, phase)
        };
        Fragment {
            position,
            rising,
            scale: envelope * (1.0 - phase.powi(6)),
        }
    })
}

fn envelope(age: f32, length: f32) -> f32 {
    let rise = (age / 0.15).clamp(0.0, 1.0);
    let fall = ((length - age) / 0.3).clamp(0.0, 1.0);
    rise.min(fall)
}

fn ground(point: Vec2) -> Vec3 {
    Vec3::new(point.x, 0.0, point.y)
}

fn tick_seconds() -> f32 {
    1.0 / count_u32(TICK_RATE)
}

fn tick_time(tick: u64) -> f32 {
    let whole = u32::try_from(tick / u64::from(TICK_RATE)).unwrap_or(u32::MAX);
    let part =
        u32::try_from(tick % u64::from(TICK_RATE)).expect("remainder fits in u32");
    count_u32(whole) + count_u32(part) * tick_seconds()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "commit counts beyond 2^24 only lose sub-commit timing precision"
)]
fn count(value: usize) -> f32 {
    value as f32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "tick and piece counts beyond 2^24 only lose sub-tick precision"
)]
fn count_u32(value: u32) -> f32 {
    value as f32
}

/// Formats Unix seconds as a UTC calendar date, using Howard Hinnant's civil algorithm.
fn date(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524
        - day_of_era / 146_096)
        / 365;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "playback durations are positive and far below u32::MAX frames"
)]
pub(super) fn frames_for(duration: f32, fps: u16) -> u32 {
    ((duration * f32::from(fps)).ceil() as u32).max(2)
}

pub(super) fn preview_frame(elapsed: Duration, fps: u16) -> u32 {
    let frame = elapsed.as_nanos() * u128::from(fps) / 1_000_000_000;
    u32::try_from(frame).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline;

    fn simulation() -> Simulation {
        Simulation::new(&timeline::synthetic(), &Options::default(), &Theme::neon())
    }

    fn assert_clearance(sim: &Simulation) {
        let state = &sim.state;
        let eye = state.focus + view_direction() * state.distance;
        let tops = state.file_tops();
        let highest = screen_bounds(eye, state.focus, &tops).1.y;
        for avatar in state.avatars.iter().filter(|avatar| avatar.presence > 0.0) {
            let radius = state.geometry.avatar_radius * sim.scale();
            let corners = box_corners(Vec3::splat(-radius), Vec3::splat(radius))
                .map(|corner| avatar.position + corner);
            let projected: Vec<_> = project(eye, state.focus, &corners)
                .map(|point| point.expect("avatar bounds are in front of the camera"))
                .collect();
            let lowest = projected
                .iter()
                .map(|point| point.y)
                .fold(f32::MAX, f32::min);
            assert!(
                lowest > highest + 2.0 * AVATAR_CLEARANCE,
                "tick {}: avatar {lowest}, file {highest}, distance {}",
                state.tick,
                state.distance
            );
            assert!(
                projected.iter().all(|point| point.x.abs() <= 0.9201
                    && (-0.8001..=0.7201).contains(&point.y)),
                "tick {}: avatar outside fit margins: {projected:?}",
                state.tick
            );
            let label = [avatar.position + state.geometry.avatar_label_offset];
            let label = project(eye, state.focus, &label)
                .next()
                .flatten()
                .expect("label is in front of the camera");
            assert!(
                label.y - 2.0 * state.geometry.label_height / 720.0
                    > highest + 2.0 * AVATAR_CLEARANCE
            );
            assert!(avatar.position.y >= MIN_AVATAR_HEIGHT);
        }
        assert!(
            project(eye, state.focus, &tops).all(|point| point.is_some()),
            "files are in front of the camera"
        );
    }

    #[test]
    fn synthetic_avatars_clear_file_tops_during_growth_and_rewind() {
        for start in [0, 8] {
            let mut sim = Simulation::new(
                &timeline::synthetic(),
                &Options {
                    start,
                    ..Options::default()
                },
                &Theme::neon(),
            );
            for _ in 0..2 {
                for frame in 0..frames_for(sim.duration(), 60) {
                    sim.advance_to(frame, 60);
                    assert_clearance(&sim);
                }
            }
        }
    }

    #[test]
    fn projection_check_rejects_fixed_height() {
        let mut sim = simulation();
        sim.advance_to(13, 2);
        assert_clearance(&sim);
        let state = &sim.state;
        let eye = state.focus + view_direction() * state.distance;
        let highest = screen_bounds(eye, state.focus, &state.file_tops()).1.y;
        let points: Vec<_> = state
            .avatars
            .iter()
            .map(|avatar| {
                Vec3::new(avatar.position.x, MIN_AVATAR_HEIGHT, avatar.position.z)
            })
            .collect();
        let lowest = screen_bounds(eye, state.focus, &points).0.y;
        assert!(lowest < highest + 2.0 * AVATAR_CLEARANCE);
    }

    #[test]
    fn lift_and_fit_settle_without_camera_drift() {
        let mut sim = simulation();
        sim.advance_to(13, 2);
        sim.state.layout.settle(SETTLE_TICKS, tick_seconds());
        for _ in 0..600 {
            sim.state.place_view(tick_seconds());
        }
        let (focus, distance) = sim.view();
        for _ in 0..120 {
            sim.state.place_view(tick_seconds());
            assert_clearance(&sim);
        }
        assert!((sim.view().0 - focus).length() < 0.01);
        assert!((sim.view().1 - distance).abs() < 0.01);
    }

    #[test]
    fn clearance_uses_themes_tile_and_avatar_dimensions() {
        let mut theme = Theme::neon();
        theme.file_frame_position.y = 0.4;
        theme.file_frame_scale *= 2.0;
        theme.avatar_radius = 0.5;
        theme.label_height = 28.0;
        let mut sim =
            Simulation::new(&timeline::synthetic(), &Options::default(), &theme);
        for frame in 0..frames_for(sim.duration(), 30) {
            sim.advance_to(frame, 30);
            assert_clearance(&sim);
        }
    }

    #[test]
    fn playback_is_independent_of_frame_rate_and_replay() {
        let mut thirty = simulation();
        let mut sixty = simulation();
        thirty.advance_to(90, 30);
        sixty.advance_to(180, 60);
        let positions = |sim: &Simulation| -> Vec<_> {
            sim.layout()
                .files()
                .map(|(id, file)| (id, file.position))
                .collect()
        };
        assert_eq!(positions(&thirty), positions(&sixty));
        let reference = positions(&thirty);
        thirty.advance_to(200, 30);
        thirty.advance_to(90, 30);
        assert_eq!(positions(&thirty), reference);
    }

    #[test]
    fn crowded_commit_aggregates_beams() {
        let mut sim = simulation();
        let crowded = LEAD_IN + sim.commit_seconds * 7.0 + 0.5;
        sim.advance_to(frame_at(crowded), 30);
        let beams = sim.beams();
        assert!(!beams.is_empty());
        assert!(beams.len() <= BEAM_LIMIT * 2, "{} beams", beams.len());
        assert!(
            sim.labels()
                .iter()
                .any(|label| label.text.ends_with("fixtures/"))
        );
    }

    #[test]
    fn whole_history_plays_out() {
        let mut sim = simulation();
        let end = frame_at(sim.duration());
        sim.advance_to(end, 30);
        assert_eq!(sim.commit(), sim.total());
        assert!(sim.beams().is_empty());
        let paths: Vec<_> = sim
            .layout()
            .files()
            .map(|(_, file)| file.path.as_str())
            .collect();
        assert!(paths.contains(&"src/scene/layout.rs"));
        assert!(!paths.contains(&"src/old.rs"));
        assert!(!paths.contains(&"docs/guide.md"));
    }

    #[test]
    fn starting_mid_history_shows_prior_files_settled() {
        let options = Options {
            start: 8,
            commits: Some(2),
            ..Options::default()
        };
        let sim = Simulation::new(&timeline::synthetic(), &options, &Theme::neon());
        assert_eq!(sim.commit(), 8);
        assert!(sim.layout().present_files() > 60);
        assert!(sim.layout().files().all(|(_, file)| file.presence >= 1.0));
    }

    #[test]
    fn dates_are_utc_calendar_days() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(date(1_700_000_000), "2023-11-14");
    }

    #[test]
    fn preview_frame_counts_elapsed_frames() {
        assert_eq!(preview_frame(Duration::from_secs(1), 30), 30);
        assert_eq!(preview_frame(Duration::from_millis(1500), 60), 90);
    }

    fn frame_at(seconds: f32) -> u32 {
        let frames = (seconds * 30.0).ceil();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "test times are short and positive"
        )]
        let frames = frames as u32;
        frames
    }
}
