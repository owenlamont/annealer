use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub file_min: Vec3,
    pub file_max: Vec3,
    pub avatar_radius: f32,
    pub avatar_label_offset: Vec3,
    pub label_height: f32,
}

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
    pub(super) directories: Vec<Vec<StandardMaterial>>,
    pub(super) directory_node_scale: Vec3,
    pub(super) directory_edge_widths: Vec<f32>,
    pub(super) directory_edge_height: f32,
    pub(super) file_frame_position: Vec3,
    pub(super) file_frame_scale: Vec3,
    pub(super) file_face_position: Vec3,
    pub(super) file_face_scale: Vec3,
    pub(super) file_spine_position: Vec3,
    pub(super) file_spine_scale: Vec3,
    pub(super) label: Color,
    pub(super) label_background: Color,
    pub(super) heading: Color,
    pub(super) label_emphasis: Color,
    pub(super) label_landmark: Color,
    pub(super) avatar_radius: f32,
    pub(super) avatar_label_offset: Vec3,
    pub(super) label_height: f32,
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
            directories: [
                200.0, 20.0, 275.0, 135.0, 335.0, 65.0, 225.0, 165.0, 305.0, 95.0,
                355.0, 245.0,
            ]
            .map(|hue| {
                [(0.65, 0.52), (0.62, 0.49), (0.58, 0.46), (0.54, 0.43)]
                    .map(|(saturation, lightness)| {
                        solid(Color::hsl(hue, saturation, lightness))
                    })
                    .to_vec()
            })
            .to_vec(),
            directory_node_scale: Vec3::new(0.24, 0.04, 0.24),
            directory_edge_widths: vec![0.20, 0.15, 0.11, 0.08],
            directory_edge_height: 0.025,
            file_frame_position: Vec3::new(0.0, 0.05, 0.0),
            file_frame_scale: Vec3::new(0.46, 0.06, 0.58),
            file_face_position: Vec3::new(0.0, 0.085, 0.0),
            file_face_scale: Vec3::new(0.34, 0.01, 0.46),
            file_spine_position: Vec3::new(-0.13, 0.09, 0.0),
            file_spine_scale: Vec3::new(0.05, 0.02, 0.40),
            label: Color::srgb(0.72, 0.83, 0.9),
            label_background: Color::srgba(0.008, 0.016, 0.03, 0.72),
            heading: Color::srgb(0.65, 0.8, 0.87),
            label_emphasis: Color::srgb(0.88, 0.95, 1.0),
            label_landmark: Color::srgb(0.62, 0.86, 0.95),
            avatar_radius: 0.22,
            avatar_label_offset: Vec3::new(0.3, 0.3, 0.0),
            label_height: 20.0,
        }
    }

    pub(super) fn geometry(&self) -> Geometry {
        let (file_min, file_max) = [
            (self.file_frame_position, self.file_frame_scale),
            (self.file_face_position, self.file_face_scale),
            (self.file_spine_position, self.file_spine_scale),
            (Vec3::new(0.03, 0.09, -0.12), Vec3::new(0.26, 0.015, 0.03)),
        ]
        .into_iter()
        .fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low, high), (position, scale)| {
                (
                    low.min(position - scale * 0.5),
                    high.max(position + scale * 0.5),
                )
            },
        );
        Geometry {
            file_min,
            file_max,
            avatar_radius: self.avatar_radius,
            avatar_label_offset: self.avatar_label_offset,
            label_height: self.label_height,
        }
    }

    pub(super) fn directory_edge_width(&self, depth: u16) -> f32 {
        self.directory_edge_widths[usize::from(depth.saturating_sub(1))
            .min(self.directory_edge_widths.len() - 1)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trunks_are_wider_and_spines_have_pale_backing() {
        let theme = Theme::neon();
        assert!(theme.directory_edge_width(1) > theme.directory_edge_width(4));
        assert_eq!(
            theme.directory_edge_width(4),
            theme.directory_edge_width(u16::MAX)
        );
        let face_left = theme.file_face_position.x - theme.file_face_scale.x * 0.5;
        let face_right = theme.file_face_position.x + theme.file_face_scale.x * 0.5;
        let spine_half_width = theme.file_spine_scale.x * 0.5;
        assert!(theme.file_spine_position.x - spine_half_width > face_left);
        assert!(theme.file_spine_position.x + spine_half_width < face_right);
    }
}
