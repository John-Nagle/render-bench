//
//  Rendering benchmark for Rend3/WGPU.
//
//  Based roughly on scene-viewer from Rend3.
//
//  Shared memory threaded targets only - no Android.
//
use anyhow::{anyhow, Context, Error};
use glam::{DVec2, Mat3A, Mat4, UVec2, Vec3, Vec3A};
use pico_args::Arguments;
use rend3::{
    types::{
        Backend, Camera, CameraProjection, DirectionalLight, DirectionalLightHandle, SampleCount,
        Texture, TextureFormat,
    },
    util::typedefs::FastHashMap,
    Renderer, RendererProfile,
};
use rend3_framework::{lock, Mutex};
use rend3_routine::{base::BaseRenderGraph, skybox::SkyboxRoutine};
use std::time::Instant;
use std::{collections::HashMap, hash::BuildHasher, path::Path, sync::Arc, time::Duration};
use wgpu_profiler::GpuTimerScopeResult;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, WindowEvent},
    window::{Fullscreen, WindowBuilder},
};

use super::citybuilder::{CityBuilder, CityParams};
use super::platform;
//
//  Constants
//
//  Names of all the assets files.
const SKYBOX_TEXTURES_DIR: &str = "/resources/skybox";
const CITY_TEXTURES_DIR: &str = "/resources/city";
const CITY_TEXTURES: [(&str, &str, &str); 6] = [
    ("brick", "redbrick_albedo.png", "redbrick_normal.png"),
    ("ground", "cobblestone_albedo.png", "cobblestone_normal.png"),
    ("roof", "roof_gravel_albedo.png", "roof_gravel_normal.png"),
    ("floor", "terracotta_floor_albedo.png", "terracotta_floor_normal.png"),
    ("ceiling","ceiling_tiles_albedo.png", "ceiling_tiles_normal.png"),
    ("stone", "white_stone_albedo.png", "white_stone_normal.png"),
];

/// Load all faces of a skybox image. Output bytes as one big RGBA-ordered image.
fn load_skybox_images(prefix: &str, filenames: &[&str]) -> Result<((u32, u32), Vec<u8>), Error> {
    println!("Loading skybox textures.");
    use image::{EncodableLayout, GenericImageView};
    let mut v = Vec::new(); // accum bytes
    let mut dims: Option<(u32, u32)> = None; // size of objects
    if filenames.len() != 6 {
        return Err(anyhow!("Skybox image set must have exactly 6 images"));
    }
    for filename in filenames {
        let full_pathname = format!("{}/{}", prefix, filename);
        let img = image::open(&full_pathname)
            .with_context(|| format!("Skybox file {}", full_pathname))?;
        //  Check that all images have the same dimensions
        match dims {
            Some(dims) => {
                if img.dimensions() != dims {
                    return Err(anyhow!(
                        "Skybox image {} is {:?} but others are {:?}",
                        filename,
                        img.dimensions(),
                        dims
                    ));
                }
            }
            None => {
                dims = Some(img.dimensions());
            }
        }
        v.extend_from_slice(img.to_rgba8().as_bytes()); // load image
    }
    Ok((dims.unwrap(), v))
}

/// Load the skybox from individual images.
fn load_skybox(renderer: &Renderer, skybox_routine: &Mutex<SkyboxRoutine>) -> Result<(), Error> {
    let prefix = env!("CARGO_MANIFEST_DIR").to_owned() + SKYBOX_TEXTURES_DIR; // filename prefix
    let skybox_files: [&str; 6] = [
        "right.jpg",
        "left.jpg",
        "top.jpg",
        "bottom.jpg",
        "front.jpg",
        "back.jpg",
    ];
    let (dims, image) = load_skybox_images(&prefix, &skybox_files)?; // Combine into one big texture
    let handle = renderer.add_texture_cube(Texture {
        format: TextureFormat::Rgba8UnormSrgb,
        size: UVec2::new(dims.0, dims.1),
        data: image,
        label: Some("background".into()),
        mip_count: rend3::types::MipmapCount::ONE,
        mip_source: rend3::types::MipmapSource::Uploaded,
    });
    //  Finally set skybox
    skybox_routine.lock().set_background_texture(Some(handle));
    Ok(())
}

fn button_pressed<Hash: BuildHasher>(map: &HashMap<u32, bool, Hash>, key: u32) -> bool {
    map.get(&key).map_or(false, |b| *b)
}

fn extract_backend(value: &str) -> Result<Backend, &'static str> {
    Ok(match value.to_lowercase().as_str() {
        "vulkan" | "vk" => Backend::Vulkan,
        "dx12" | "12" => Backend::Dx12,
        "dx11" | "11" => Backend::Dx11,
        "metal" | "mtl" => Backend::Metal,
        "opengl" | "gl" => Backend::Gl,
        _ => return Err("unknown backend"),
    })
}

fn extract_mode(value: &str) -> Result<rend3::RendererProfile, &'static str> {
    Ok(match value.to_lowercase().as_str() {
        "legacy" | "c" | "cpu" => rend3::RendererProfile::CpuDriven,
        "modern" | "g" | "gpu" => rend3::RendererProfile::GpuDriven,
        _ => return Err("unknown rendermode"),
    })
}

fn extract_msaa(value: &str) -> Result<SampleCount, &'static str> {
    Ok(match value {
        "1" => SampleCount::One,
        "4" => SampleCount::Four,
        _ => return Err("invalid msaa count"),
    })
}

fn extract_vec3(value: &str) -> Result<Vec3, &'static str> {
    let mut res = [0.0_f32, 0.0, 0.0];
    let split: Vec<_> = value.split(',').enumerate().collect();

    if split.len() != 3 {
        return Err("Directional lights are defined with 3 values");
    }

    for (idx, inner) in split {
        let inner = inner.trim();

        res[idx] = inner.parse().map_err(|_| "Cannot parse direction number")?;
    }
    Ok(Vec3::from(res))
}

fn option_arg<T>(result: Result<Option<T>, pico_args::Error>) -> Option<T> {
    match result {
        Ok(o) => o,
        Err(pico_args::Error::Utf8ArgumentParsingFailed { value, cause }) => {
            eprintln!("{}: '{}'\n\n{}", cause, value, HELP);
            std::process::exit(1);
        }
        Err(pico_args::Error::OptionWithoutAValue(value)) => {
            eprintln!("{} flag needs an argument", value);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("{:?}", e);
            std::process::exit(1);
        }
    }
}

const HELP: &str = "\
render-bench

Exercise Rend3 and WGPU with a complex, changing scene.

usage: render-bench --options 

Meta:
  --help            This menu.

Rendering:
  -b --backend                 Choose backend to run on ('vk', 'dx12', 'dx11', 'metal', 'gl').
  -d --device                  Choose device to run on (case insensitive device substring).
  -p --profile                 Choose rendering profile to use ('cpu', 'gpu').
  --msaa <level>               Level of antialiasing (either 1 or 4). Default 1.

Windowing:
  --absolute-mouse             Interpret the relative mouse coordinates as absolute. Useful when using things like VNC.
  --fullscreen                 Open the window in borderless fullscreen.

Assets:
  --normal-y-down                        Interpret all normals as having the DirectX convention of Y down. Defaults to Y up.
  --directional-light <x,y,z>            Create a directional light pointing towards the given coordinates.
  --directional-light-intensity <value>  All lights created by the above flag have this intensity. Defaults to 4.
  --ambient <value>                      Set the value of the minimum ambient light. This will be treated as white light of this intensity. Defaults to 0.1.
  --scale <scale>                        Scale all objects loaded by this factor. Defaults to 1.0.
  --shadow-distance <value>              Distance from the camera there will be directional shadows. Lower values means higher quality shadows. Defaults to 300.

Controls:
  --walk <speed>               Walk speed (speed without holding shift) in units/second (typically meters). Default 10.
  --run  <speed>               Run speed (speed while holding shift) in units/second (typically meters). Default 50.
";

struct SceneViewer {
    //  Parameters
    absolute_mouse: bool,
    desired_backend: Option<Backend>,
    desired_device_name: Option<String>,
    desired_profile: Option<RendererProfile>,
    walk_speed: f32,
    run_speed: f32,
    directional_light_direction: Option<Vec3>,
    directional_light_intensity: f32,
    directional_light: Option<DirectionalLightHandle>,
    ambient_light_level: f32,
    samples: SampleCount,

    fullscreen: bool,

    scancode_status: FastHashMap<u32, bool>,
    camera_pitch: f32,
    camera_yaw: f32,
    camera_location: Vec3A,
    previous_profiling_stats: Option<Vec<GpuTimerScopeResult>>,
    timestamp_last_second: Instant,
    timestamp_last_frame: Instant,
    frame_times: histogram::Histogram,
    last_mouse_delta: Option<DVec2>,

    grabber: Option<rend3_framework::Grabber>,

    //  Model
    city_builder: CityBuilder, // what we get to look at
}
impl SceneViewer {
    pub fn new() -> Self {
        let mut args = Arguments::from_vec(std::env::args_os().skip(1).collect());

        // Meta
        let help = args.contains(["-h", "--help"]);

        // Rendering
        let desired_backend =
            option_arg(args.opt_value_from_fn(["-b", "--backend"], extract_backend));
        let desired_device_name: Option<String> =
            option_arg(args.opt_value_from_str(["-d", "--device"]))
                .map(|s: String| s.to_lowercase());
        let desired_mode = option_arg(args.opt_value_from_fn(["-p", "--profile"], extract_mode));
        let samples =
            option_arg(args.opt_value_from_fn("--msaa", extract_msaa)).unwrap_or(SampleCount::One);

        // Windowing
        let absolute_mouse: bool = args.contains("--absolute-mouse");
        let fullscreen = args.contains("--fullscreen");

        // Assets
        let directional_light_direction =
            match option_arg(args.opt_value_from_fn("--directional-light", extract_vec3)) {
                Some(v) => Some(v),
                None => Some(Vec3::new(-1.0, -1.0, -1.0)), // reasonable default sunlight direction
            };
        let directional_light_intensity: f32 =
            option_arg(args.opt_value_from_str("--directional-light-intensity")).unwrap_or(4.0);
        let ambient_light_level: f32 =
            option_arg(args.opt_value_from_str("--ambient")).unwrap_or(0.10);

        // Controls
        let walk_speed = args.value_from_str("--walk").unwrap_or(10.0_f32);
        let run_speed = args.value_from_str("--run").unwrap_or(50.0_f32);

        // Free args
        let remaining = args.finish();

        if !remaining.is_empty() {
            eprint!("Unknown arguments:");
            for flag in remaining {
                eprint!(" '{}'", flag.to_string_lossy());
            }
            eprintln!("\n");

            eprintln!("{}", HELP);
            std::process::exit(1);
        }
        //  Model

        if help {
            eprintln!("{}", HELP);
            std::process::exit(1);
        }

        //  Parameters for city building
        let building_count = 1; // ***TEMP***
        let city_params = CityParams::new(
            building_count,
            env!("CARGO_MANIFEST_DIR").to_owned() + CITY_TEXTURES_DIR,
            CITY_TEXTURES.to_vec(),
        );

        Self {
            absolute_mouse,
            desired_backend,
            desired_device_name,
            desired_profile: desired_mode,
            walk_speed,
            run_speed,
            directional_light_direction,
            directional_light_intensity,
            directional_light: None,
            ambient_light_level,
            samples,

            fullscreen,

            scancode_status: FastHashMap::default(),
            camera_pitch: -std::f32::consts::FRAC_PI_8,
            camera_yaw: std::f32::consts::FRAC_PI_4,
            camera_location: Vec3A::new(3.0, 2.0, 3.0),
            previous_profiling_stats: None,
            timestamp_last_second: Instant::now(),
            timestamp_last_frame: Instant::now(),
            frame_times: histogram::Histogram::new(),
            last_mouse_delta: None,

            grabber: None,
            //  Model parameters
            city_builder: CityBuilder::new(city_params), // our model
        }
    }
}
impl rend3_framework::App for SceneViewer {
    const HANDEDNESS: rend3::types::Handedness = rend3::types::Handedness::Right;

    fn create_iad<'a>(
        &'a mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<rend3::InstanceAdapterDevice>> + 'a>,
    > {
        Box::pin(async move {
            Ok(rend3::create_iad(
                self.desired_backend,
                self.desired_device_name.clone(),
                self.desired_profile,
                None,
            )
            .await?)
        })
    }

    fn sample_count(&self) -> SampleCount {
        self.samples
    }

    fn scale_factor(&self) -> f32 {
        // Android has very low memory bandwidth, so lets run internal buffers at half
        // res by default
        cfg_if::cfg_if! {
            if #[cfg(target_os = "android")] {
                0.5
            } else {
                1.0
            }
        }
    }

    fn setup<'a>(
        &'a mut self,
        window: &'a winit::window::Window,
        renderer: &'a Arc<Renderer>,
        routines: &'a Arc<rend3_framework::DefaultRoutines>,
        _surface_format: rend3::types::TextureFormat,
    ) {
        self.grabber = Some(rend3_framework::Grabber::new(window));

        const SUN_SHADOW_DISTANCE: f32 = 300.0;
        if let Some(direction) = self.directional_light_direction {
            self.directional_light = Some(renderer.add_directional_light(DirectionalLight {
                color: Vec3::splat(1.0),
                intensity: self.directional_light_intensity,
                direction,
                distance: SUN_SHADOW_DISTANCE,
            }));
        }

        let renderer = Arc::clone(renderer);
        let routines = Arc::clone(routines);
        load_skybox(&renderer, &routines.skybox).unwrap(); // load the background skybox
        let thread_count = 2; // ***TEMP***
        self.city_builder.start(thread_count, renderer); // start up the city generator
    }

    fn handle_event(
        &mut self,
        window: &winit::window::Window,
        renderer: &Arc<rend3::Renderer>,
        routines: &Arc<rend3_framework::DefaultRoutines>,
        base_rendergraph: &BaseRenderGraph,
        surface: Option<&Arc<rend3::types::Surface>>,
        resolution: UVec2,
        event: rend3_framework::Event<'_, ()>,
        control_flow: impl FnOnce(winit::event_loop::ControlFlow),
    ) {
        match event {
            Event::MainEventsCleared => {
                profiling::scope!("MainEventsCleared");
                let now = Instant::now();

                let delta_time = now - self.timestamp_last_frame;
                self.frame_times
                    .increment(delta_time.as_micros() as u64)
                    .unwrap();

                let elapsed_since_second = now - self.timestamp_last_second;
                if elapsed_since_second > Duration::from_secs(1) {
                    let count = self.frame_times.entries();
                    println!(
                        "{:0>5} frames over {:0>5.2}s. \
                        Min: {:0>5.2}ms; \
                        Average: {:0>5.2}ms; \
                        95%: {:0>5.2}ms; \
                        99%: {:0>5.2}ms; \
                        Max: {:0>5.2}ms; \
                        StdDev: {:0>5.2}ms",
                        count,
                        elapsed_since_second.as_secs_f32(),
                        self.frame_times.minimum().unwrap() as f32 / 1_000.0,
                        self.frame_times.mean().unwrap() as f32 / 1_000.0,
                        self.frame_times.percentile(95.0).unwrap() as f32 / 1_000.0,
                        self.frame_times.percentile(99.0).unwrap() as f32 / 1_000.0,
                        self.frame_times.maximum().unwrap() as f32 / 1_000.0,
                        self.frame_times.stddev().unwrap() as f32 / 1_000.0,
                    );
                    self.timestamp_last_second = now;
                    self.frame_times.clear();
                }

                self.timestamp_last_frame = now;

                let rotation = Mat3A::from_euler(
                    glam::EulerRot::XYZ,
                    -self.camera_pitch,
                    -self.camera_yaw,
                    0.0,
                )
                .transpose();
                let forward = -rotation.z_axis;
                let up = rotation.y_axis;
                let side = -rotation.x_axis;
                let velocity = if button_pressed(&self.scancode_status, platform::Scancodes::SHIFT)
                {
                    self.run_speed
                } else {
                    self.walk_speed
                };
                if button_pressed(&self.scancode_status, platform::Scancodes::W) {
                    self.camera_location += forward * velocity * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, platform::Scancodes::S) {
                    self.camera_location -= forward * velocity * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, platform::Scancodes::A) {
                    self.camera_location += side * velocity * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, platform::Scancodes::D) {
                    self.camera_location -= side * velocity * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, platform::Scancodes::Q) {
                    self.camera_location += up * velocity * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, platform::Scancodes::Z) {
                    self.camera_location -= up * velocity * delta_time.as_secs_f32();
                }
                if button_pressed(&self.scancode_status, platform::Scancodes::ESCAPE) {
                    self.grabber.as_mut().unwrap().request_ungrab(window);
                }
                if button_pressed(&self.scancode_status, platform::Scancodes::P) {
                    // write out gpu side performance info into a trace readable by chrome://tracing
                    if let Some(ref stats) = self.previous_profiling_stats {
                        println!("Outputing gpu timing chrome trace to profile.json");
                        wgpu_profiler::chrometrace::write_chrometrace(
                            Path::new("profile.json"),
                            stats,
                        )
                        .unwrap();
                    } else {
                        println!("No gpu timing trace available, either timestamp queries are unsupported or not enough frames have elapsed yet!");
                    }
                }

                window.request_redraw()
            }
            Event::RedrawRequested(_) => {
                let view = Mat4::from_euler(
                    glam::EulerRot::XYZ,
                    -self.camera_pitch,
                    -self.camera_yaw,
                    0.0,
                );
                let view = view * Mat4::from_translation((-self.camera_location).into());

                renderer.set_camera_data(Camera {
                    projection: CameraProjection::Perspective {
                        vfov: 60.0,
                        near: 0.1,
                    },
                    view,
                });

                // Get a frame
                let frame = rend3::util::output::OutputFrame::Surface {
                    surface: Arc::clone(surface.unwrap()),
                };
                // Lock all the routines
                let pbr_routine = lock(&routines.pbr);
                let mut skybox_routine = lock(&routines.skybox);
                let tonemapping_routine = lock(&routines.tonemapping);

                // Ready up the renderer
                let (cmd_bufs, ready) = renderer.ready();
                // Ready up the routines
                skybox_routine.ready(renderer);

                // Build a rendergraph
                let mut graph = rend3::graph::RenderGraph::new();

                // Add the default rendergraph
                base_rendergraph.add_to_graph(
                    &mut graph,
                    &ready,
                    &pbr_routine,
                    Some(&skybox_routine),
                    &tonemapping_routine,
                    resolution,
                    self.samples,
                    Vec3::splat(self.ambient_light_level).extend(1.0),
                );

                // Dispatch a render using the built up rendergraph!
                self.previous_profiling_stats = graph.execute(renderer, frame, cmd_bufs, &ready);
                // mark the end of the frame for tracy/other profilers
                profiling::finish_frame!();
            }
            Event::WindowEvent {
                event: WindowEvent::Focused(focus),
                ..
            } => {
                if !focus {
                    self.grabber.as_mut().unwrap().request_ungrab(window);
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                scancode, state, ..
                            },
                        ..
                    },
                ..
            } => {
                log::info!("WE scancode {:x}", scancode);
                self.scancode_status.insert(
                    scancode,
                    match state {
                        ElementState::Pressed => true,
                        ElementState::Released => false,
                    },
                );
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        button: MouseButton::Left,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                let grabber = self.grabber.as_mut().unwrap();

                if !grabber.grabbed() {
                    grabber.request_grab(window);
                }
            }
            Event::DeviceEvent {
                event:
                    DeviceEvent::MouseMotion {
                        delta: (delta_x, delta_y),
                        ..
                    },
                ..
            } => {
                if !self.grabber.as_ref().unwrap().grabbed() {
                    return;
                }

                const TAU: f32 = std::f32::consts::PI * 2.0;

                let mouse_delta = if self.absolute_mouse {
                    let prev = self.last_mouse_delta.replace(DVec2::new(delta_x, delta_y));
                    if let Some(prev) = prev {
                        (DVec2::new(delta_x, delta_y) - prev) / 4.0
                    } else {
                        return;
                    }
                } else {
                    DVec2::new(delta_x, delta_y)
                };

                self.camera_yaw -= (mouse_delta.x / 1000.0) as f32;
                self.camera_pitch -= (mouse_delta.y / 1000.0) as f32;
                if self.camera_yaw < 0.0 {
                    self.camera_yaw += TAU;
                } else if self.camera_yaw >= TAU {
                    self.camera_yaw -= TAU;
                }
                self.camera_pitch = self
                    .camera_pitch
                    .max(-std::f32::consts::FRAC_PI_2 + 0.0001)
                    .min(std::f32::consts::FRAC_PI_2 - 0.0001);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                self.city_builder.stop(); // shut down other threads
                control_flow(winit::event_loop::ControlFlow::Exit);
            }
            _ => {}
        }
    }
}

#[cfg_attr(
    target_os = "android",
    ndk_glue::main(backtrace = "on", logger(level = "debug"))
)]
pub fn viewer() {
    let app = SceneViewer::new();

    let mut builder = WindowBuilder::new()
        .with_title("render-bench")
        .with_maximized(true);
    if app.fullscreen {
        builder = builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }

    rend3_framework::start(app, builder);
}
