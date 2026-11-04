use std::collections::HashMap;

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

use crate::model::{self, Simulation};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const LABELS: usize = 72;
const GRID_LINES: i16 = 20;

const NEON: [Color; 6] = [
    Color::srgb(0.25, 0.92, 0.95),
    Color::srgb(1.0, 0.6, 0.25),
    Color::srgb(0.95, 0.35, 0.85),
    Color::srgb(0.55, 1.0, 0.45),
    Color::srgb(1.0, 0.9, 0.35),
    Color::srgb(0.55, 0.6, 1.0),
];

#[derive(Resource)]
pub(super) struct Frame {
    pub index: u32,
    pub fps: u16,
}

#[derive(Resource)]
pub(super) struct Playback(pub Simulation);

#[derive(Resource)]
struct SceneTarget(RenderTarget);

#[derive(Component)]
struct SceneCamera;

#[derive(Component)]
struct Label;

#[derive(Component)]
struct Caption;

#[derive(Component)]
struct FileVisual {
    halo: Entity,
    author: Option<usize>,
}

#[derive(Resource)]
struct Palette {
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
    beam: Handle<Mesh>,
    paper: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
    grid: Handle<StandardMaterial>,
    descending: Handle<StandardMaterial>,
    rising: Handle<StandardMaterial>,
    spines: Vec<Handle<StandardMaterial>>,
    avatars: Vec<Handle<StandardMaterial>>,
    glass: Vec<Handle<StandardMaterial>>,
}

impl Palette {
    fn new(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let cube = meshes.add(Cuboid::default());
        let sphere = meshes.add(Sphere::new(0.22));
        let beam = meshes.add(Cylinder::new(0.035, 1.0));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let mut solid = |color| {
            materials.add(StandardMaterial {
                base_color: color,
                unlit: true,
                ..default()
            })
        };
        let paper = solid(Color::srgb(0.64, 0.76, 0.85));
        let edge = solid(Color::srgb(0.09, 0.2, 0.28));
        let grid = solid(Color::srgb(0.045, 0.09, 0.13));
        let descending = solid(NEON[0]);
        let rising = solid(NEON[1]);
        let spines = [
            Color::srgb(1.0, 0.55, 0.25),
            Color::srgb(0.35, 0.65, 1.0),
            Color::srgb(0.85, 0.9, 0.95),
            Color::srgb(0.45, 0.95, 0.55),
            Color::srgb(1.0, 0.85, 0.3),
            Color::srgb(0.9, 0.4, 0.9),
            Color::srgb(0.45, 0.5, 0.58),
        ]
        .map(&mut solid)
        .to_vec();
        let avatars = NEON.map(&mut solid).to_vec();
        let glass = NEON
            .map(|color| {
                materials.add(StandardMaterial {
                    base_color: color.with_alpha(0.32),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                })
            })
            .to_vec();
        Self {
            cube,
            sphere,
            beam,
            paper,
            edge,
            grid,
            descending,
            rising,
            spines,
            avatars,
            glass,
        }
    }

    fn spine(&self, extension: &str) -> Handle<StandardMaterial> {
        let index = match extension {
            "rs" | "c" | "cpp" | "h" | "go" => 0,
            "py" | "pyi" | "ipynb" => 1,
            "md" | "rst" | "txt" | "html" => 2,
            "toml" | "yaml" | "yml" | "json" | "lock" | "cfg" | "ini" => 3,
            "js" | "ts" | "tsx" | "jsx" | "mjs" => 4,
            "sh" | "ps1" | "nix" | "snap" => 5,
            _ => 6,
        };
        self.spines[index].clone()
    }
}

#[derive(Resource, Default)]
struct Pools {
    files: HashMap<u64, Entity>,
    beams: Vec<Entity>,
    fragments: Vec<Entity>,
    edges: Vec<Entity>,
    avatars: Vec<Entity>,
    grid: Vec<Entity>,
}

fn app(simulation: Simulation, title: String) -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.012, 0.023, 0.04)))
        .insert_resource(Playback(simulation))
        .insert_resource(Title(title))
        .init_resource::<Pools>()
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

#[derive(Resource)]
struct Title(String);

pub(super) fn preview(simulation: Simulation, title: String) {
    let mut app = app(simulation, title);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Annealer — layout study".to_owned(),
            resolution: (WIDTH, HEIGHT).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(SceneTarget(RenderTarget::default()))
    .insert_resource(Frame {
        index: 0,
        fps: model::PREVIEW_FPS,
    })
    .add_systems(Update, preview_clock.before(animate));
    app.run();
}

pub(super) fn headless(
    simulation: Simulation,
    title: String,
    fps: u16,
) -> (App, Handle<Image>) {
    let mut app = app(simulation, title);
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
            Transform::from_xyz(6.0, 9.5, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
            SceneCamera,
        ))
        .id();
    let palette = Palette::new(world);
    let floor =
        world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgb(0.02, 0.036, 0.06),
                unlit: true,
                ..default()
            });
    world.spawn((
        Mesh3d(palette.cube.clone()),
        MeshMaterial3d(floor),
        Transform::from_xyz(0.0, -0.2, 0.0).with_scale(Vec3::new(4000.0, 0.1, 4000.0)),
    ));
    let grid: Vec<_> = (0..(4 * GRID_LINES + 2))
        .map(|_| {
            world
                .spawn((
                    Mesh3d(palette.cube.clone()),
                    MeshMaterial3d(palette.grid.clone()),
                    Transform::default(),
                ))
                .id()
        })
        .collect();
    world.resource_mut::<Pools>().grid = grid;
    world.insert_resource(palette);
    for _ in 0..LABELS {
        world.spawn((
            Text::new(""),
            TextFont::from_font_size(14.0),
            TextColor(Color::srgb(0.72, 0.83, 0.9)),
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(px(3.0), px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.008, 0.016, 0.03, 0.72)),
            Visibility::Hidden,
            UiTargetCamera(camera),
            Label,
        ));
    }
    let title = world.resource::<Title>().0.clone();
    heading(world, camera, "ANNEALER", 28.0, 28.0, 30.0);
    heading(world, camera, &title, 28.0, 68.0, 16.0);
    let caption = heading(world, camera, " ", 28.0, 668.0, 14.0);
    world.entity_mut(caption).insert(Caption);
}

fn heading(
    world: &mut World,
    camera: Entity,
    text: &str,
    left: f32,
    top: f32,
    size: f32,
) -> Entity {
    world
        .spawn((
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
        ))
        .id()
}

fn preview_clock(world: &mut World) {
    let elapsed = world.resource::<Time<Real>>().elapsed();
    let duration = world.resource::<Playback>().0.duration();
    let fps = world.resource::<Frame>().fps;
    let frames = model::frames_for(duration, fps);
    world.resource_mut::<Frame>().index = model::preview_frame(elapsed, fps) % frames;
    if world
        .resource::<ButtonInput<KeyCode>>()
        .just_pressed(KeyCode::Escape)
    {
        world.write_message(AppExit::Success);
    }
}

fn animate(world: &mut World) {
    let frame = world.resource::<Frame>();
    let (index, fps) = (frame.index, frame.fps);
    world.resource_scope(|world, mut playback: Mut<Playback>| {
        playback.0.advance_to(index, fps);
        world.resource_scope(|world, palette: Mut<Palette>| {
            world.resource_scope(|world, mut pools: Mut<Pools>| {
                let simulation = &playback.0;
                sync_files(world, &palette, &mut pools, simulation);
                sync_structure(world, &palette, &mut pools, simulation);
                sync_activity(world, &palette, &mut pools, simulation);
                place_camera(world, simulation);
            });
        });
    });
}

fn sync_files(
    world: &mut World,
    palette: &Palette,
    pools: &mut Pools,
    simulation: &Simulation,
) {
    let layout = simulation.layout();
    pools.files.retain(|id, entity| {
        let keep = layout.file(*id).is_some();
        if !keep {
            world.despawn(*entity);
        }
        keep
    });
    for (id, file) in layout.files() {
        let entity = *pools
            .files
            .entry(id)
            .or_insert_with(|| spawn_file(world, palette, file.extension()));
        let position = Vec3::new(file.position.x, 0.0, file.position.y);
        if let Some(mut transform) = world.get_mut::<Transform>(entity) {
            transform.translation = position;
            transform.scale = Vec3::splat(file.presence.max(0.001));
        }
        let visual = world
            .get::<FileVisual>(entity)
            .expect("file entities are visuals");
        let (halo, current) = (visual.halo, visual.author);
        let glowing = file.heat > 0.03;
        if glowing && current != Some(file.author) {
            world.entity_mut(halo).insert(MeshMaterial3d(
                palette.glass[file.author % NEON.len()].clone(),
            ));
            world
                .get_mut::<FileVisual>(entity)
                .expect("file entities are visuals")
                .author = Some(file.author);
        }
        if let Some(mut transform) = world.get_mut::<Transform>(halo) {
            let grow = 1.0 + 0.45 * file.heat;
            transform.scale = Vec3::new(0.5 * grow, 0.02, 0.62 * grow);
        }
        set_visible(world, halo, glowing);
    }
}

fn spawn_file(world: &mut World, palette: &Palette, extension: &str) -> Entity {
    let paper = block(
        world,
        &palette.cube,
        &palette.paper,
        Vec3::new(0.0, 0.05, 0.0),
        Vec3::new(0.46, 0.06, 0.58),
    );
    let spine = block(
        world,
        &palette.cube,
        &palette.spine(extension),
        Vec3::new(-0.19, 0.09, 0.0),
        Vec3::new(0.05, 0.02, 0.5),
    );
    let ink = block(
        world,
        &palette.cube,
        &palette.edge,
        Vec3::new(0.03, 0.09, -0.12),
        Vec3::new(0.26, 0.015, 0.03),
    );
    let halo = block(
        world,
        &palette.cube,
        &palette.glass[0],
        Vec3::new(0.0, 0.01, 0.0),
        Vec3::ONE,
    );
    world.entity_mut(halo).insert(Visibility::Hidden);
    world
        .spawn((
            Transform::default(),
            Visibility::default(),
            FileVisual { halo, author: None },
        ))
        .add_children(&[paper, spine, ink, halo])
        .id()
}

fn sync_structure(
    world: &mut World,
    palette: &Palette,
    pools: &mut Pools,
    simulation: &Simulation,
) {
    let layout = simulation.layout();
    let edges: Vec<_> = layout
        .dirs()
        .filter_map(|(_, dir)| {
            let parent = layout.dir(dir.parent?);
            Some((
                ground(parent.position) - Vec3::Y * 0.05,
                ground(dir.position) - Vec3::Y * 0.05,
            ))
        })
        .collect();
    let pool = grow(
        world,
        &mut pools.edges,
        edges.len(),
        &palette.beam,
        &palette.edge,
    );
    for (entity, &(from, to)) in pool.iter().zip(&edges) {
        place_beam(world, *entity, from, to, 0.6);
    }
    hide_rest(world, pool, edges.len());
    let (focus, distance) = simulation.view();
    let extent = distance * 0.5;
    let spacing = 2.0_f32.powf((extent / 4.0).log2().ceil().max(0.0));
    let centre = (focus / spacing).round() * spacing;
    let half = spacing * f32::from(GRID_LINES);
    let thickness = 0.012 * spacing.sqrt();
    for (index, &entity) in pools.grid.iter().enumerate() {
        let line = f32::from(
            i16::try_from(index).expect("grid pool is small") % (2 * GRID_LINES + 1)
                - GRID_LINES,
        );
        let along_x = index
            >= usize::try_from(2 * GRID_LINES + 1).expect("grid size is positive");
        let (translation, scale) = if along_x {
            (
                Vec3::new(centre.x, -0.13, centre.z + line * spacing),
                Vec3::new(2.0 * half, 0.01, thickness),
            )
        } else {
            (
                Vec3::new(centre.x + line * spacing, -0.13, centre.z),
                Vec3::new(thickness, 0.01, 2.0 * half),
            )
        };
        if let Some(mut transform) = world.get_mut::<Transform>(entity) {
            *transform = Transform::from_translation(translation).with_scale(scale);
        }
    }
}

fn sync_activity(
    world: &mut World,
    palette: &Palette,
    pools: &mut Pools,
    simulation: &Simulation,
) {
    let avatars = simulation.avatars();
    while pools.avatars.len() < avatars.len() {
        let index = pools.avatars.len();
        let entity = world
            .spawn((
                Mesh3d(palette.sphere.clone()),
                MeshMaterial3d(palette.avatars[index % NEON.len()].clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();
        pools.avatars.push(entity);
    }
    for (index, &entity) in pools.avatars.iter().enumerate() {
        let avatar = avatars.get(index);
        let presence = avatar.map_or(0.0, |avatar| avatar.presence);
        if let (Some(avatar), Some(mut transform)) =
            (avatar, world.get_mut::<Transform>(entity))
        {
            transform.translation = avatar.position;
            transform.scale = Vec3::splat((presence * simulation.scale()).max(0.001));
        }
        set_visible(world, entity, presence > 0.0);
    }
    let beams = simulation.beams();
    let mut pool = std::mem::take(&mut pools.beams);
    while pool.len() < beams.len() {
        pool.push(
            world
                .spawn((
                    Mesh3d(palette.beam.clone()),
                    MeshMaterial3d(palette.glass[0].clone()),
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id(),
        );
    }
    for (entity, beam) in pool.iter().zip(&beams) {
        world.entity_mut(*entity).insert(MeshMaterial3d(
            palette.glass[beam.author % NEON.len()].clone(),
        ));
        place_beam(
            world,
            *entity,
            beam.from,
            beam.to,
            beam.width * simulation.scale().sqrt(),
        );
    }
    hide_rest(world, &pool, beams.len());
    pools.beams = pool;
    let fragments = simulation.fragments();
    let mut pool = std::mem::take(&mut pools.fragments);
    while pool.len() < fragments.len() {
        let entity = block(
            world,
            &palette.cube,
            &palette.descending,
            Vec3::ZERO,
            Vec3::ONE,
        );
        pool.push(entity);
    }
    for (entity, fragment) in pool.iter().zip(&fragments) {
        let material = if fragment.rising {
            &palette.rising
        } else {
            &palette.descending
        };
        world
            .entity_mut(*entity)
            .insert(MeshMaterial3d(material.clone()));
        if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
            *transform = Transform::from_translation(fragment.position).with_scale(
                Vec3::new(0.12, 0.035, 0.2)
                    * (fragment.scale * simulation.scale().sqrt()).max(0.001),
            );
        }
        set_visible(world, *entity, fragment.scale > 0.0);
    }
    hide_rest(world, &pool, fragments.len());
    pools.fragments = pool;
}

fn place_camera(world: &mut World, simulation: &Simulation) {
    let (focus, distance) = simulation.view();
    let mut query = world.query_filtered::<&mut Transform, With<SceneCamera>>();
    if let Ok(mut transform) = query.single_mut(world) {
        *transform =
            Transform::from_translation(focus + model::view_direction() * distance)
                .looking_at(focus, Vec3::Y);
    }
}

fn grow<'a>(
    world: &mut World,
    pool: &'a mut Vec<Entity>,
    needed: usize,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
) -> &'a [Entity] {
    while pool.len() < needed {
        pool.push(
            world
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id(),
        );
    }
    pool
}

fn hide_rest(world: &mut World, pool: &[Entity], used: usize) {
    for &entity in &pool[used.min(pool.len())..] {
        set_visible(world, entity, false);
    }
}

fn set_visible(world: &mut World, entity: Entity, visible: bool) {
    let wanted = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if let Some(mut visibility) = world.get_mut::<Visibility>(entity)
        && *visibility != wanted
    {
        *visibility = wanted;
    }
}

fn place_beam(world: &mut World, entity: Entity, from: Vec3, to: Vec3, width: f32) {
    let delta = to - from;
    if let Some(mut transform) = world.get_mut::<Transform>(entity) {
        *transform = Transform::from_translation((from + to) * 0.5)
            .with_rotation(Quat::from_rotation_arc(
                Vec3::Y,
                delta.normalize_or(Vec3::Y),
            ))
            .with_scale(Vec3::new(
                width.max(0.001),
                delta.length().max(0.001),
                width.max(0.001),
            ));
    }
    set_visible(world, entity, width > 0.0);
}

fn block(
    world: &mut World,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
) -> Entity {
    world
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position).with_scale(size),
            Visibility::default(),
        ))
        .id()
}

fn ground(point: Vec2) -> Vec3 {
    Vec3::new(point.x, 0.0, point.y)
}

fn labels(world: &mut World) {
    let mut camera_query =
        world.query_filtered::<(&Camera, &GlobalTransform), With<SceneCamera>>();
    let Ok((camera, transform)) = camera_query.single(world) else {
        return;
    };
    let (camera, transform) = (camera.clone(), *transform);
    let playback = world.resource::<Playback>();
    let caption = playback.0.caption();
    let mut wanted = playback.0.labels();
    wanted.sort_by_key(|label| label.kind);
    let mut placed: Vec<Rect> = Vec::new();
    let mut chosen = Vec::new();
    for label in wanted {
        let Ok(point) = camera.world_to_viewport(&transform, label.position) else {
            continue;
        };
        let width = 8.0
            + 8.6
                * f32::from(
                    u16::try_from(label.text.chars().count()).unwrap_or(u16::MAX),
                );
        let rect = Rect::from_corners(point, point + Vec2::new(width, 20.0));
        if rect.min.x < 0.0
            || rect.min.y < 0.0
            || rect.max.x > 1280.0
            || rect.max.y > 720.0
            || placed.iter().any(|other| !other.intersect(rect).is_empty())
        {
            continue;
        }
        placed.push(rect);
        chosen.push((point, label.text, label.kind != model::LabelKind::Landmark));
        if chosen.len() == LABELS {
            break;
        }
    }
    let mut query = world.query_filtered::<(&mut Text, &mut Node, &mut Visibility, &mut TextColor), With<Label>>();
    let mut chosen = chosen.into_iter();
    for (mut text, mut node, mut visibility, mut color) in query.iter_mut(world) {
        if let Some((point, label, emphasis)) = chosen.next() {
            if text.0 != label {
                text.0 = label;
            }
            node.left = px(point.x);
            node.top = px(point.y);
            color.0 = if emphasis {
                Color::srgb(0.88, 0.95, 1.0)
            } else {
                Color::srgb(0.62, 0.86, 0.95)
            };
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    let mut query = world.query_filtered::<&mut Text, With<Caption>>();
    if let Ok(mut text) = query.single_mut(world)
        && text.0 != caption
    {
        text.0 = caption;
    }
}
