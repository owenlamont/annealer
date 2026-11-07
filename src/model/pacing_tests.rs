use super::*;
use crate::timeline::{FileChange, Identity};

fn event(day: i64, changes: Vec<FileChange>) -> Event {
    Event {
        seconds: day * 86_400,
        author: Identity {
            name: b"Ada".to_vec(),
            email: Vec::new(),
        },
        contributor: None,
        changes,
    }
}

fn changes(dir: &str, count: usize, kind: &Kind) -> Vec<FileChange> {
    (0..count)
        .map(|index| FileChange {
            path: format!("{dir}/{index}"),
            kind: kind.clone(),
            insertions: 1,
            removals: 0,
        })
        .collect()
}

fn growth() -> Vec<Event> {
    let mut events: Vec<_> = (0..40)
        .map(|day| event(day, changes("a", 1, &Kind::Modified)))
        .collect();
    events[0].changes = changes("a", 1, &Kind::Added);
    events[0].changes.extend(changes("z", 1, &Kind::Added));
    events[20].changes = changes("a/deep", 80, &Kind::Added);
    events
}

fn simulation(events: Vec<Event>, options: &Options) -> Simulation {
    Simulation::new(events, options, &Theme::neon(), &Mappings::default())
}

#[test]
fn mid_gap_rate_change_preserves_plan_time() {
    let mut sim = simulation(growth(), &Options::default());
    sim.advance_to(585, 60);
    let now = sim.seconds();
    let before = sim.plan_seconds(now);
    sim.change_rate(0.5);
    let after = sim.plan_seconds(now);
    assert!((before - after).abs() < 0.0001, "{before} -> {after}");
}

#[test]
fn accelerated_playback_bounds_retained_directory_speed() {
    for factor in [1.0, 0.125] {
        let mut sim = simulation(growth(), &Options::default());
        sim.change_rate(factor);
        let mut previous = BTreeMap::new();
        let mut worst = 0.0_f32;
        let frames = frames_for(sim.duration(), 60).unwrap();
        for _ in 0..frames {
            sim.tick_layout();
            let mut current = BTreeMap::new();
            for (id, dir) in sim.layout().dirs() {
                if let Some(position) = previous.get(&id) {
                    worst = worst.max(dir.position.distance(*position) * 60.0);
                }
                current.insert(id, dir.position);
            }
            previous = current;
        }
        eprintln!("factor {factor}: maximum retained-directory speed {worst}");
        assert!(worst <= 3.001, "factor {factor}: {worst} units/s");
        assert_eq!(sim.commit(), sim.total());
    }
}

#[test]
fn accelerated_playback_reserves_fading_directory_space() {
    let mut first = changes("a", 100, &Kind::Added);
    first.extend(changes("b", 1, &Kind::Added));
    let events = vec![
        event(0, first),
        event(1, changes("a", 100, &Kind::Deleted)),
        event(2, changes("b", 100, &Kind::Added)),
        event(3, changes("b", 1, &Kind::Modified)),
    ];
    let options = Options {
        seconds_per_day: 100.0,
        max_idle_seconds: 200.0,
        ..Options::default()
    };
    for (factor, change_tick) in [(1.0, 0), (1.0 / 1024.0, 0), (2.0, 6036)] {
        let mut sim = simulation(events.clone(), &options);
        for _ in 0..change_tick {
            sim.tick_layout();
        }
        if change_tick > 0 {
            assert!(sim.layout().files().any(|(_, file)| {
                file.state == crate::layout::Presence::Leaving && file.presence > 0.0
            }));
        }
        sim.change_rate(factor);
        let frames = frames_for(sim.duration(), 60).unwrap();
        let mut fading = false;
        for frame in change_tick..frames {
            sim.tick_layout();
            fading |= sim.layout().files().any(|(_, file)| {
                file.state == crate::layout::Presence::Leaving && file.presence > 0.0
            });
            let dirs: Vec<_> = sim.layout().dirs().collect();
            for (index, (_, a)) in dirs.iter().enumerate() {
                for (_, b) in &dirs[index + 1..] {
                    assert!(
                        a.position.distance(b.position) + 0.0001
                            >= a.radius() + b.radius(),
                        "factor {factor}: overlap {} / {} at frame {frame}",
                        a.path,
                        b.path
                    );
                }
            }
        }
        assert!(fading);
        assert_eq!(sim.commit(), sim.total());
        assert!(
            sim.layout()
                .files()
                .all(|(_, file)| !file.path.starts_with("a/"))
        );
    }
}

#[test]
fn repeated_rate_changes_preserve_clock_history_and_playback_second_bounds() {
    let mut sim = simulation(growth(), &Options::default());
    sim.advance_to(585, 60);
    for factor in [4.0, 0.5, 0.5, 0.5, 8.0, 0.125] {
        let now = sim.seconds();
        let history: Vec<_> = (0..=585)
            .map(|tick| sim.plan_seconds(tick_time(tick)))
            .collect();
        let before = sim.plan_seconds(now);
        sim.change_rate(factor);
        assert!((sim.plan_seconds(now) - before).abs() < 0.0001);
        for (tick, &expected) in history.iter().enumerate() {
            let time = tick_time(u64::try_from(tick).unwrap());
            assert!((sim.plan_seconds(time) - expected).abs() < 0.0001);
        }
        for (playback, plan) in sim
            .clock_starts
            .windows(2)
            .zip(sim.clock_plan_starts.windows(2))
        {
            let elapsed = playback[1] - playback[0];
            let advance = plan[1] - plan[0];
            assert!(elapsed > 0.0);
            assert!(advance >= 0.0 && advance <= elapsed + 0.0001);
        }
        sim.advance_to(586, 60);
        let first = sim.layout().forecast();
        sim.advance_to(0, 60);
        sim.advance_to(586, 60);
        assert_eq!(sim.layout().forecast(), first);
    }
    sim.restart();
    assert_eq!(sim.seconds(), 0.0);
    assert_eq!(sim.plan_seconds(0.0), 0.0);
    assert_eq!(&sim.clock_starts[1..], sim.starts);
    assert_eq!(&sim.clock_plan_starts[1..], sim.plan_starts);
}

#[test]
fn slowing_then_speeding_changes_pacing_within_the_plan_limit() {
    let mut sim = simulation(growth(), &Options::default());
    sim.advance_to(45, 60);
    sim.change_rate(2.0);
    assert!(sim.starts[1] > 1.0 && sim.starts[1] < 1.25);
    let before = sim.plan_seconds(sim.seconds());
    sim.advance_to(46, 60);
    let advance = sim.plan_seconds(sim.seconds()) - before;
    assert!(advance < tick_seconds() && advance > tick_seconds() / 2.0);
    let anchor = sim.plan_seconds(sim.seconds());
    let rate = sim.plan_rate(sim.seconds());
    sim.change_rate(0.5);
    assert_eq!(sim.plan_seconds(sim.seconds()), anchor);
    assert_eq!(sim.plan_rate(sim.seconds()), rate);
    sim.advance_to(47, 60);
    assert!(sim.plan_rate(sim.seconds()) > rate);
    sim.change_rate(0.5);
    assert!(sim.caption().contains("requested (layout limit)"));
}

#[test]
fn live_rate_transitions_bound_directory_acceleration() {
    let mut sim = simulation(growth(), &Options::default());
    let mut previous: BTreeMap<usize, Vec2> = BTreeMap::new();
    let mut velocities: BTreeMap<usize, Vec2> = BTreeMap::new();
    let mut worst = 0.0_f32;
    for tick in 0..2400 {
        if tick % 60 == 0 {
            let rate = sim.plan_rate(sim.seconds());
            sim.change_rate(if tick % 120 == 0 { 2.0 } else { 0.5 });
            assert_eq!(sim.plan_rate(sim.seconds()), rate);
        }
        sim.tick_layout();
        let mut current = BTreeMap::new();
        let mut next_velocities = BTreeMap::new();
        for (id, dir) in sim.layout().dirs() {
            if let Some(position) = previous.get(&id) {
                let velocity = (dir.position - *position) / tick_seconds();
                assert!(velocity.length() <= 3.001);
                if let Some(previous_velocity) = velocities.get(&id) {
                    worst = worst
                        .max((velocity - *previous_velocity).length() / tick_seconds());
                }
                next_velocities.insert(id, velocity);
            }
            current.insert(id, dir.position);
        }
        previous = current;
        velocities = next_velocities;
    }
    eprintln!("live-rate maximum retained-directory acceleration {worst}");
    assert!(worst <= 48.01, "{worst} units/s²");
}
