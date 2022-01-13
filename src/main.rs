#![feature(stdarch)]

extern crate sobol_burley as sobol;

use std::{
    collections::BinaryHeap,
    sync::{Arc, Mutex, RwLock,
        atomic::{Ordering, AtomicUsize, AtomicBool}
    },
    time::{Instant, Duration},
    io::Write
};

mod bsdf;
mod camera;
mod color;
mod integrator;
mod math;
mod sampling;
mod scene;
mod shape;
mod spectrum;
mod tile;
mod types;

use std::env;
use camera::Camera;
use scene::Scene;
use tile::TileData;
use sampling::Sampler;
use spectrum::Wavelength;
use math::Ray;
use integrator::Integrator;

const WIDTH: usize = 1024;
const HEIGHT: usize = 1024;
const TOTAL_SPP: usize = 4;

//#[cfg(feature = "hwss")]
type CurrentIntegrator = integrator::path_integrator::PathIntegrator;
//#[cfg(not(feature = "hwss"))]
//type CurrentIntegrator = integrator::swss_naive::SwssNaive;
//type CurrentIntegrator = integrator::hwss_naive::HwssNaive;

pub struct Render {
    pub width: usize,
    pub height: usize,
    pub spp: usize,
    pub scene: Scene,
    pub camera: Camera,
    pub buffer: RwLock<Vec<(f32, f32, f32)>>,
    pub integrator: CurrentIntegrator,
}

fn main() {

    let mut total_spp = TOTAL_SPP;
    let mut output_file_name = String::from("");

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        total_spp = (&args[1]).parse::<usize>().unwrap();
    }
    if args.len() > 2 {
        output_file_name.push_str("results/");
        output_file_name.push_str(&args[2]);
        output_file_name.push_str(".png");
    }

    let render = Arc::new(Render {
        width: WIDTH,
        height: HEIGHT,
        spp: total_spp,
        integrator: CurrentIntegrator::default(),
        scene: scene::Scene::dispersion(),
        buffer: RwLock::new(vec![(0.0, 0.0, 0.0); WIDTH * HEIGHT]),
        camera: Camera::new(
            math::Point3::new(0.0, 0.0, 0.0),
            (WIDTH as f32) / (HEIGHT as f32),
        ),
    });
    
    let tile_priorities = Arc::new(Mutex::new(
        // TODO: Make this nice
        (0..)
            .map(|idx| TileData::new(&render, idx))
            .take_while(|t| t.is_some())
            .map(|t| t.unwrap())
            .collect::<BinaryHeap<TileData>>(),
    ));
    
    let num_threads = std::env::var("NTHREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(num_cpus::get);
    
    if output_file_name.len() > 0 {
        do_render_png(render, tile_priorities, num_threads, output_file_name);
    }
    else {
        do_render_progressive(render, tile_priorities, num_threads);
    }
}

fn do_render_png(
    render: Arc<Render>,
    tile_priorities: Arc<Mutex<BinaryHeap<TileData>>>,
    num_threads: usize,
    output_file_name: String
) {
    println!(
        "Starting render, {}x{}@{}spp to save as PNG file",
        render.width, render.height, render.spp
    );

    let start = Instant::now();

    let threads = (0..num_threads)
        .map(|_| {
            let tile_priorities = tile_priorities.clone();
            let render = render.clone();
            std::thread::spawn(move || loop {
                let popped = tile_priorities.lock().unwrap().pop();
                match popped {
                    Some(tile) => {
                        tile.render(&render);
                    }
                    None => {
                        break;
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        thread.join().unwrap();
    }

    let elapsed = start.elapsed().as_secs_f32();
    println!(
        "Done in {}s ({}m ray/s)",
        elapsed,
        ((render.spp * WIDTH * HEIGHT) as f32) / (1_000_000.0 * elapsed),
    );

    // read finished render buffer
    let lock_result: 
            std::sync::LockResult<std::sync::RwLockReadGuard<'_, std::vec::Vec<(f32, f32, f32)>>>
            = render.buffer.read();
    let buffer_guarded_vec = lock_result.ok().unwrap();
    let buffer_float = &*buffer_guarded_vec;

    // choose file to save image to
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(output_file_name)
        .unwrap();
    let ref mut w = std::io::BufWriter::new(file);

    // configure the encoder
    let mut encoder = png::Encoder::new(w, WIDTH as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_trns(vec!(0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8));
    encoder.set_source_gamma(png::ScaledFloat::from_scaled(45455));
    encoder.set_source_gamma(png::ScaledFloat::new(1.0 / 2.2));
    let source_chromaticities = png::SourceChromaticities::new(
        (0.31270, 0.32900),
        (0.64000, 0.33000),
        (0.30000, 0.60000),
        (0.15000, 0.06000)
    );
    encoder.set_source_chromaticities(source_chromaticities);
    let mut writer = encoder.write_header().unwrap();

    // convert the f32 (R,G,B) values to u8 format in order [R,G,B,A]
    let mut buffer: Vec<u8> = vec![0; WIDTH * HEIGHT * 4];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let index = y * WIDTH + x;
            let pixel = buffer_float[index];

            // convert from [0,1] to [0,255]
            let r: f32 = pixel.0.max(0.0).min(1.0) * 255.0;
            let g: f32 = pixel.1.max(0.0).min(1.0) * 255.0;
            let b: f32 = pixel.2.max(0.0).min(1.0) * 255.0;

            // convert to u8
            buffer[4*index + 0] = r as u8;
            buffer[4*index + 1] = g as u8;
            buffer[4*index + 2] = b as u8;
            buffer[4*index + 3] = 255;
        } 
    }

    // write to file
    writer.write_image_data(&buffer);
}

fn do_render_progressive(
    render: Arc<Render>,
    tile_priorities: Arc<Mutex<BinaryHeap<TileData>>>,
    num_threads: usize,
) {
    use minifb::{Key, Window, WindowOptions};

    let mut window = Window::new(
        "Iris",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            ..Default::default()
        },
    )
    .expect("failed to create window");

    static SAMPLES_TAKEN: AtomicUsize = AtomicUsize::new(0);
    static DONE: AtomicBool = AtomicBool::new(false);

    println!(
        "Starting render, {}x{}@{}spp in progressive mode",
        render.width, render.height, render.spp
    );

    let start = Instant::now();

    for _ in 0..num_threads {
        let tile_priorities = tile_priorities.clone();
        let render = render.clone();
        std::thread::spawn(move || loop {
            let popped = tile_priorities.lock().unwrap().pop();
            match popped {
                Some(tile) => {
                    let samples_before = tile.remaining_samples;
                    let tile = tile.render(&render);
                    let samples_after = tile.remaining_samples;
                    SAMPLES_TAKEN.fetch_add(
                        (samples_before - samples_after) * tile.width * tile.height,
                        Ordering::Relaxed,
                    );

                    if samples_after > 0 {
                        tile_priorities.lock().unwrap().push(tile);
                    }
                }
                None => {
                    if !DONE.swap(true, Ordering::Relaxed) {
                        let elapsed = start.elapsed().as_secs_f32();
                        println!(
                            "Done in {}s ({}m ray/s)",
                            elapsed,
                            ((render.spp * WIDTH * HEIGHT) as f32) / (1_000_000.0 * elapsed),
                        );
                    }
                    break;
                }
            }
        });
    }

    let target_rate = std::time::Duration::from_micros(100000); // 10fps
    window.limit_update_rate(None);

    let mut prev_time = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if !DONE.load(Ordering::Relaxed) {
            let progress =
            SAMPLES_TAKEN.load(Ordering::Relaxed) as f32 / (render.spp * WIDTH * HEIGHT) as f32;
            print!("Progress: {:>5.2}%\r", 100.0 * progress);
            std::io::stdout().flush().unwrap();
        }

        // TODO: Use some kind of sleeping mutex so that we only update the screen when
        // the render buffer has changed?
        let target_rate = target_rate.as_secs_f64();
        let current_time = Instant::now();
        let delta = current_time
            .saturating_duration_since(prev_time)
            .as_secs_f64();

        if delta < target_rate {
            let sleep_time = target_rate - delta;
            if sleep_time > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(sleep_time));
            }
        }

        prev_time = Instant::now();

        //let buffer = render.buffer.read().unwrap();
        let lock_result: 
            std::sync::LockResult<std::sync::RwLockReadGuard<'_, std::vec::Vec<(f32, f32, f32)>>>
            = render.buffer.read();
        let buffer_guarded_vec = lock_result.ok().unwrap();
        let buffer_float = &*buffer_guarded_vec;

        // convert the f32 RGB values to u32 format 0RGB
        //let mut buffer: [u32; WIDTH*HEIGHT] = [0; WIDTH*HEIGHT]; stack overflow for 512x512
        let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let index = y * WIDTH + x;
                let pixel = buffer_float[index];

                let mut result: u32 = 0;
                let r: f32 = pixel.0.min(1.0) * 255.0;
                let g: f32 = pixel.1.min(1.0) * 255.0;
                let b: f32 = pixel.2.min(1.0) * 255.0;
                //println!("read pixel ({}, {}, {})", r, g, b);

                result += r as u32;
                result = result << 8;
                result += g as u32;
                result = result << 8;
                result += b as u32;

                buffer[index] = result;
            } 
        }

        window
            .update_with_buffer(&buffer, render.width, render.height)
            .expect("failed to update window buffer with pixel data");
    }
}
