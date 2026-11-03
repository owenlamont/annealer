use std::f32::consts::FRAC_PI_2;

use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    prelude::*,
    render::{
        RenderPlugin,
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::TextureFormat,
        view::{DebandDither, Tonemapping},
    },
    ui::UiSystems,
    window::ExitCondition,
    winit::WinitPlugin,
};

use crate::model::{self, FILES};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

#[derive(Resource)]
pub(super) struct Frame {
    pub index: u16,
    pub fps: u16,
}

#[derive(Resource)]
struct SceneTarget(RenderTarget);

#[derive(Component)]
struct FileLabel(usize);

#[derive(Component)]
enum Animated {
    Avatar {
        second: bool,
    },
    Beam {
        file: usize,
        second: bool,
    },
    File(usize),
    Fragment {
        file: usize,
        rising: bool,
        offset: f32,
    },
}

struct Palette {
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
    beam: Handle<Mesh>,
    paper: Handle<StandardMaterial>,
    ink: Handle<StandardMaterial>,
    brass: Handle<StandardMaterial>,
    cyan: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    amber_glass: Handle<StandardMaterial>,
    ground: Handle<StandardMaterial>,
    grid: Handle<StandardMaterial>,
}

impl Palette {
    fn new(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let cube = meshes.add(Cuboid::default());
        let sphere = meshes.add(Sphere::new(0.22));
        let beam = meshes.add(Cylinder::new(0.04, 1.0));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let mut material = |color| {
            materials.add(StandardMaterial {
                base_color: color,
                unlit: true,
                ..default()
            })
        };
        Self {
            cube,
            sphere,
            beam,
            paper: material(Color::srgb(0.64, 0.76, 0.85)),
            ink: material(Color::srgb(0.1, 0.22, 0.3)),
            brass: material(Color::srgb(1.0, 0.65, 0.3)),
            cyan: material(Color::srgb(0.25, 0.92, 0.95)),
            ground: material(Color::srgb(0.025, 0.045, 0.075)),
            grid: material(Color::srgb(0.055, 0.11, 0.16)),
            glass: materials.add(StandardMaterial {
                base_color: Color::srgba(0.2, 0.85, 0.95, 0.26),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            amber_glass: materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.55, 0.2, 0.3),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
        }
    }
}

fn app() -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.023, 0.04)))
        .insert_resource(Frame {
            index: 0,
            fps: model::PREVIEW_FPS,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, animate)
        .add_systems(
            PostUpdate,
            labels
                .after(bevy::camera::CameraUpdateSystems)
                .after(bevy::transform::TransformSystems::Propagate)
                .before(UiSystems::Layout),
        );
    app
}

pub(super) fn preview() {
    let mut app = app();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Annealer — material study".to_owned(),
            resolution: (WIDTH, HEIGHT).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(SceneTarget(RenderTarget::default()))
    .add_systems(Update, preview_clock.before(animate));
    app.run();
}

pub(super) fn headless(fps: u16) -> (App, Handle<Image>) {
    let mut app = app();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    );
    let mut image =
        Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(SceneTarget(target.clone().into()))
        .insert_resource(Frame { index: 0, fps });
    (app, target)
}

fn setup(world: &mut World) {
    let target = world.resource::<SceneTarget>().0.clone();
    let camera = world
        .spawn((
            Camera3d::default(),
            target,
            Tonemapping::None,
            DebandDither::Disabled,
            Transform::from_xyz(6.0, 9.5, 12.0)
                .looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        ))
        .id();
    let palette = Palette::new(world);
    block(
        world,
        &palette,
        &palette.ground,
        Vec3::new(0.0, -0.16, 0.0),
        Vec3::new(15.0, 0.12, 11.0),
    );
    for line in -6_i16..=6 {
        block(
            world,
            &palette,
            &palette.grid,
            Vec3::new(f32::from(line), -0.09, 0.0),
            Vec3::new(0.012, 0.012, 10.0),
        );
    }
    for line in -4_i16..=4 {
        block(
            world,
            &palette,
            &palette.grid,
            Vec3::new(0.0, -0.09, f32::from(line)),
            Vec3::new(14.0, 0.012, 0.012),
        );
    }
    spawn_files(world, &palette, camera);
    spawn_activity(world, &palette);
    heading(world, camera, "ANNEALER", 28.0, 28.0, 30.0);
    heading(
        world,
        camera,
        "A small history, taking shape",
        28.0,
        68.0,
        16.0,
    );
    heading(world, camera, "01   /   MATERIAL STUDY", 28.0, 668.0, 14.0);
}

fn block(
    world: &mut World,
    palette: &Palette,
    material: &Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
) -> Entity {
    world
        .spawn((
            Mesh3d(palette.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position).with_scale(size),
        ))
        .id()
}

fn spawn_files(world: &mut World, palette: &Palette, camera: Entity) {
    for (index, file) in FILES.iter().enumerate() {
        let parent = world
            .spawn((
                Transform::from_translation(file.position.into()),
                Animated::File(index),
            ))
            .id();
        let paper = block(
            world,
            palette,
            &palette.paper,
            Vec3::new(0.0, 0.08, 0.0),
            Vec3::new(0.92, 0.08, 1.15),
        );
        let spine = block(
            world,
            palette,
            &palette.brass,
            Vec3::new(-0.4, 0.13, 0.0),
            Vec3::new(0.07, 0.025, 1.0),
        );
        world.entity_mut(parent).add_children(&[paper, spine]);
        for line in 0_i16..4 {
            let width = if line == 3 { 0.29 } else { 0.58 };
            let ink = block(
                world,
                palette,
                &palette.ink,
                Vec3::new(0.0, 0.13, f32::from(line) * 0.2 - 0.32),
                Vec3::new(width, 0.02, 0.035),
            );
            world.entity_mut(parent).add_child(ink);
        }
        world.spawn((
            Text::new(file.name),
            TextFont::from_font_size(15.0),
            TextColor(Color::srgb(0.72, 0.83, 0.9)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTargetCamera(camera),
            FileLabel(index),
        ));
    }
}

fn spawn_activity(world: &mut World, palette: &Palette) {
    world.spawn((
        Mesh3d(palette.sphere.clone()),
        MeshMaterial3d(palette.cyan.clone()),
        Transform::default(),
        Animated::Avatar { second: false },
    ));
    for index in 0..8 {
        world.spawn((
            Mesh3d(palette.beam.clone()),
            MeshMaterial3d(palette.glass.clone()),
            Transform::default(),
            Animated::Beam {
                file: index,
                second: false,
            },
        ));
    }
    world.spawn((
        Mesh3d(palette.sphere.clone()),
        MeshMaterial3d(palette.brass.clone()),
        Transform::default(),
        Animated::Avatar { second: true },
    ));
    for file in [0, 2, 5, 7] {
        world.spawn((
            Mesh3d(palette.beam.clone()),
            MeshMaterial3d(palette.amber_glass.clone()),
            Transform::default(),
            Animated::Beam { file, second: true },
        ));
    }
    for (file, rising) in [(0, false), (3, false), (5, true)] {
        for fragment in 0_u16..3 {
            let entity = block(
                world,
                palette,
                if rising {
                    &palette.brass
                } else {
                    &palette.cyan
                },
                Vec3::ZERO,
                Vec3::new(0.12, 0.035, 0.2),
            );
            world.entity_mut(entity).insert(Animated::Fragment {
                file,
                rising,
                offset: f32::from(fragment) / 3.0,
            });
        }
    }
}

fn heading(
    world: &mut World,
    camera: Entity,
    text: &str,
    left: f32,
    top: f32,
    size: f32,
) {
    world.spawn((
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(Color::srgb(0.65, 0.8, 0.87)),
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            ..default()
        },
        UiTargetCamera(camera),
    ));
}

fn preview_clock(world: &mut World) {
    let elapsed = world.resource::<Time<Real>>().elapsed();
    world.resource_mut::<Frame>().index = model::preview_frame(elapsed);
    if world
        .resource::<ButtonInput<KeyCode>>()
        .just_pressed(KeyCode::Escape)
    {
        world.write_message(AppExit::Success);
    }
}

fn animate(world: &mut World) {
    let frame = world.resource::<Frame>();
    let sample = model::sample(frame.index, frame.fps);
    let mut query = world.query::<(&Animated, &mut Transform)>();
    for (animated, mut transform) in query.iter_mut(world) {
        match *animated {
            Animated::Avatar { second } => {
                transform.translation = avatar(&sample, second);
            }
            Animated::Beam { file, second } => {
                let avatar = avatar(&sample, second);
                let target = Vec3::from(FILES[file].position) + Vec3::Y * 0.16;
                let delta = target - avatar;
                *transform = Transform::from_translation((avatar + target) * 0.5)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, delta.normalize()))
                    .with_scale(Vec3::new(1.0, delta.length(), 1.0));
            }
            Animated::File(file) => {
                let scale = file_scale(file, &sample);
                transform.scale = Vec3::splat(scale.max(0.001));
            }
            Animated::Fragment {
                file,
                rising,
                offset,
            } => {
                let progress = (sample.seconds * 0.65 + offset).fract();
                let height = if rising { progress } else { 1.0 - progress };
                transform.translation = Vec3::from(FILES[file].position)
                    + Vec3::new(0.2, 0.25 + height * 2.4, -0.1);
                transform.rotation = Quat::from_rotation_y(progress * FRAC_PI_2);
            }
        }
    }
}

fn file_scale(file: usize, sample: &model::Sample) -> f32 {
    match file {
        8 => sample.created,
        9 => sample.deleted,
        _ => 1.0,
    }
}

fn avatar(sample: &model::Sample, second: bool) -> Vec3 {
    if second {
        sample.other_avatar.into()
    } else {
        sample.avatar.into()
    }
}

fn labels(world: &mut World) {
    let frame = world.resource::<Frame>();
    let sample = model::sample(frame.index, frame.fps);
    let mut camera_query = world.query::<(&Camera, &GlobalTransform)>();
    let Ok((camera, transform)) = camera_query.single(world) else {
        return;
    };
    let positions: Vec<_> = FILES
        .iter()
        .map(|file| {
            camera.world_to_viewport(
                transform,
                Vec3::from(file.position) + Vec3::new(-0.35, 0.0, 0.9),
            )
        })
        .collect();
    let mut query = world.query::<(&FileLabel, &mut Node, &mut Visibility)>();
    for (label, mut node, mut visibility) in query.iter_mut(world) {
        if let Ok(position) = positions[label.0] {
            node.left = px(position.x);
            node.top = px(position.y);
        }
        *visibility = if file_scale(label.0, &sample) < 0.01 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}
