use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, bail};
use bevy::{
    app::PluginsState,
    prelude::{App, Handle, Image, On},
    render::{
        RenderApp,
        render_resource::{PipelineCache, PollType},
        renderer::RenderDevice,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    text::TextLayoutInfo,
};

use crate::{model, scene};

const WAIT_LIMIT: Duration = Duration::from_secs(30);

pub(super) fn run(
    directory: &Path,
    frames: u16,
    fps: u16,
    delay_ms: u16,
) -> anyhow::Result<()> {
    fs::create_dir(directory).with_context(|| {
        format!("create new export directory {}", directory.display())
    })?;
    let mut manifest = BufWriter::new(File::create(directory.join("frames.csv"))?);
    writeln!(
        manifest,
        "frame,seconds,avatar_x,avatar_y,avatar_z,created,deleted"
    )?;
    let (mut app, target) = scene::headless(fps);
    let started = Instant::now();
    while app.plugins_state() != PluginsState::Ready {
        if started.elapsed() > WAIT_LIMIT {
            bail!("renderer plugins were not ready within 30 seconds");
        }
        bevy::tasks::tick_global_task_pools_on_main_thread();
        thread::sleep(Duration::from_millis(1));
    }
    app.finish();
    app.cleanup();
    let readiness_started = Instant::now();
    loop {
        update(&mut app)?;
        if ready(&mut app) {
            break;
        }
        if readiness_started.elapsed() > WAIT_LIMIT {
            bail!("text and rendering pipelines were not ready within 30 seconds");
        }
    }
    let started = Instant::now();
    for index in 0..frames {
        app.world_mut().resource_mut::<scene::Frame>().index = index;
        let image = capture(&mut app, &target, index)?;
        let path = directory.join(format!("frame-{index:05}.png"));
        image
            .try_into_dynamic()
            .context("convert captured frame")?
            .to_rgb8()
            .save(&path)
            .with_context(|| format!("write frame {}", path.display()))?;
        let sample = model::sample(index, fps);
        writeln!(
            manifest,
            "{index},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9}",
            sample.seconds,
            sample.avatar[0],
            sample.avatar[1],
            sample.avatar[2],
            sample.created,
            sample.deleted
        )?;
        thread::sleep(Duration::from_millis(u64::from(delay_ms)));
        if index % fps == 0 || index + 1 == frames {
            eprintln!(
                "Exported {}/{} frames ({:.1}s elapsed)",
                index + 1,
                frames,
                started.elapsed().as_secs_f32()
            );
        }
    }
    manifest.flush()?;
    eprintln!("Export complete: {}", directory.display());
    Ok(())
}

fn ready(app: &mut App) -> bool {
    let world = app.world_mut();
    let mut query = world.query::<&TextLayoutInfo>();
    let mut layouts = query.iter(world).peekable();
    let labels_ready =
        layouts.peek().is_some() && layouts.all(|layout| !layout.glyphs.is_empty());
    labels_ready
        && app
            .sub_app(RenderApp)
            .world()
            .resource::<PipelineCache>()
            .waiting_pipelines()
            .next()
            .is_none()
}

fn capture(app: &mut App, target: &Handle<Image>, frame: u16) -> anyhow::Result<Image> {
    let (sender, receiver) = mpsc::channel();
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(move |event: On<ScreenshotCaptured>| {
            let _ = sender.send(event.image.clone());
        });
    let started = Instant::now();
    loop {
        update(app)?;
        match receiver.try_recv() {
            Ok(image) => return Ok(image),
            Err(mpsc::TryRecvError::Disconnected) => {
                bail!("capture channel disconnected at frame {frame}")
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if started.elapsed() > WAIT_LIMIT {
            bail!("capture timed out at frame {frame}");
        }
    }
}

fn update(app: &mut App) -> anyhow::Result<()> {
    app.update();
    app.world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .poll(PollType::Wait {
            submission_index: None,
            timeout: Some(WAIT_LIMIT),
        })
        .context("wait for GPU frame completion")?;
    Ok(())
}
