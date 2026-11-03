use std::time::Duration;

pub(super) const PREVIEW_FRAMES: u16 = 120;
pub(super) const PREVIEW_FPS: u16 = 30;

pub(super) struct File {
    pub name: &'static str,
    pub position: [f32; 3],
}

pub(super) const FILES: [File; 10] = [
    File {
        name: "main.rs",
        position: [-3.4, 0.0, -2.0],
    },
    File {
        name: "scene.rs",
        position: [-1.6, 0.0, -2.6],
    },
    File {
        name: "export.rs",
        position: [0.4, 0.0, -2.3],
    },
    File {
        name: "model.rs",
        position: [2.4, 0.0, -1.8],
    },
    File {
        name: "Cargo.toml",
        position: [-3.8, 0.0, 0.3],
    },
    File {
        name: "README.md",
        position: [-1.8, 0.0, 0.5],
    },
    File {
        name: "ci.yml",
        position: [0.4, 0.0, 0.5],
    },
    File {
        name: "layout.rs",
        position: [2.8, 0.0, 0.5],
    },
    File {
        name: "new.rs",
        position: [-1.6, 0.0, 2.6],
    },
    File {
        name: "old.rs",
        position: [1.2, 0.0, 2.8],
    },
];

#[derive(Debug, PartialEq)]
pub(super) struct Sample {
    pub seconds: f32,
    pub avatar: [f32; 3],
    pub other_avatar: [f32; 3],
    pub created: f32,
    pub deleted: f32,
}

pub(super) fn sample(frame: u16, fps: u16) -> Sample {
    let seconds = f32::from(frame) / f32::from(fps);
    Sample {
        seconds,
        avatar: [
            2.2 * (seconds * 1.3).sin(),
            3.4 + 0.2 * (seconds * 2.0).sin(),
            1.4 * (seconds * 1.3).cos(),
        ],
        other_avatar: [
            1.0 - 2.2 * (seconds * 1.3).sin(),
            4.0,
            -1.4 * (seconds * 1.3).cos(),
        ],
        created: ((seconds - 0.7) / 0.6).clamp(0.0, 1.0),
        deleted: 1.0 - ((seconds - 2.6) / 0.6).clamp(0.0, 1.0),
    }
}

pub(super) fn preview_frame(elapsed: Duration) -> u16 {
    let frame = elapsed.as_nanos() * u128::from(PREVIEW_FPS) / 1_000_000_000;
    u16::try_from(frame % u128::from(PREVIEW_FRAMES))
        .expect("preview frame modulo fits in u16")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_preserves_creation_and_deletion_order() {
        let start = sample(0, 30);
        let middle = sample(60, 30);
        let end = sample(119, 30);
        assert_eq!((start.created, start.deleted), (0.0, 1.0));
        assert_eq!((middle.created, middle.deleted), (1.0, 1.0));
        assert_eq!((end.created, end.deleted), (1.0, 0.0));
        assert_ne!(start.avatar, middle.avatar);
    }

    #[test]
    fn frame_time_matches_across_rates_and_preview() {
        assert_eq!(sample(30, 30), sample(60, 60));
        assert_eq!(preview_frame(Duration::from_secs(1)), 30);
        assert_eq!(preview_frame(Duration::from_secs(5)), 30);
        assert_eq!(preview_frame(Duration::from_secs(4)), 0);
    }
}
