use bevy::prelude::*;

#[derive(Clone)]
pub(super) struct Theme {
    pub(super) clear: Color,
    pub(super) paper: StandardMaterial,
    pub(super) edge: StandardMaterial,
    pub(super) grid: StandardMaterial,
    pub(super) descending: StandardMaterial,
    pub(super) rising: StandardMaterial,
    pub(super) floor: StandardMaterial,
    pub(super) spines: [StandardMaterial; 7],
    pub(super) avatars: [StandardMaterial; 6],
    pub(super) glass: [StandardMaterial; 6],
    pub(super) label: Color,
    pub(super) label_background: Color,
    pub(super) heading: Color,
    pub(super) label_emphasis: Color,
    pub(super) label_landmark: Color,
}

impl Theme {
    pub(super) fn neon() -> Self {
        let developers = [
            Color::srgb(0.25, 0.92, 0.95),
            Color::srgb(1.0, 0.6, 0.25),
            Color::srgb(0.95, 0.35, 0.85),
            Color::srgb(0.55, 1.0, 0.45),
            Color::srgb(1.0, 0.9, 0.35),
            Color::srgb(0.55, 0.6, 1.0),
        ];
        let solid = |base_color| StandardMaterial {
            base_color,
            unlit: true,
            ..default()
        };
        Self {
            clear: Color::srgb(0.012, 0.023, 0.04),
            paper: solid(Color::srgb(0.64, 0.76, 0.85)),
            edge: solid(Color::srgb(0.09, 0.2, 0.28)),
            grid: solid(Color::srgb(0.045, 0.09, 0.13)),
            descending: solid(developers[0]),
            rising: solid(developers[1]),
            floor: solid(Color::srgb(0.02, 0.036, 0.06)),
            spines: [
                Color::srgb(1.0, 0.55, 0.25),
                Color::srgb(0.35, 0.65, 1.0),
                Color::srgb(0.85, 0.9, 0.95),
                Color::srgb(0.45, 0.95, 0.55),
                Color::srgb(1.0, 0.85, 0.3),
                Color::srgb(0.9, 0.4, 0.9),
                Color::srgb(0.45, 0.5, 0.58),
            ]
            .map(solid),
            avatars: developers.map(solid),
            glass: developers.map(|color| StandardMaterial {
                base_color: color.with_alpha(0.32),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            label: Color::srgb(0.72, 0.83, 0.9),
            label_background: Color::srgba(0.008, 0.016, 0.03, 0.72),
            heading: Color::srgb(0.65, 0.8, 0.87),
            label_emphasis: Color::srgb(0.88, 0.95, 1.0),
            label_landmark: Color::srgb(0.62, 0.86, 0.95),
        }
    }

    pub(super) fn spine_kind(extension: &str) -> usize {
        match extension {
            "rs" | "c" | "cpp" | "h" | "go" => 0,
            "py" | "pyi" | "ipynb" => 1,
            "md" | "rst" | "txt" | "html" => 2,
            "toml" | "yaml" | "yml" | "json" | "lock" | "cfg" | "ini" => 3,
            "js" | "ts" | "tsx" | "jsx" | "mjs" => 4,
            "sh" | "ps1" | "nix" | "snap" => 5,
            _ => 6,
        }
    }
}
