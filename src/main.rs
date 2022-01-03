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

const WIDTH: usize = 1024;
const HEIGHT: usize = 1024;
const TOTAL_SPP: usize = 4;

//#[cfg(feature = "hwss")]
type CurrentIntegrator = integrator::hwss_modified::HwssModified;
//#[cfg(not(feature = "hwss"))]
//type CurrentIntegrator = integrator::swss_naive::SwssNaive;

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

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        total_spp = (&args[1]).parse::<usize>().unwrap();
    }

    let render = Arc::new(Render {
        width: WIDTH,
        height: HEIGHT,
        spp: total_spp,
        integrator: CurrentIntegrator::default(),
        scene: scene::Scene::cornell_box(),
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
    
    do_render(render, tile_priorities, num_threads);
}

#[cfg(not(feature = "progressive"))]
fn do_render(
    render: Arc<Render>,
    tile_priorities: Arc<Mutex<BinaryHeap<TileData>>>,
    num_threads: usize,
) {
    println!(
        "Starting render, {}x{}@{}spp...",
        render.width, render.height, render.spp
    );

    println!("Cancelling render because non-progressive rendering requires OpenEXR which does not work for me");

    /*

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

    // Output EXR file
    use openexr::{FrameBuffer, Header, PixelType, ScanlineOutputFile};

    let mut file = std::fs::File::create("out.exr").unwrap();
    let mut output_file = ScanlineOutputFile::new(
        &mut file,
        Header::new()
            .set_resolution(render.width as u32, render.height as u32)
            .add_channel("R", PixelType::FLOAT)
            .add_channel("G", PixelType::FLOAT)
            .add_channel("B", PixelType::FLOAT),
    )
    .unwrap();

    let pixels = render.buffer.read().unwrap();
    let mut fb = FrameBuffer::new(render.width as u32, render.height as u32);
    fb.insert_channels(&["R", "G", "B"], &pixels);
    output_file.write_pixels(&fb).unwrap();

    */
}

#[cfg(feature = "progressive")]
fn do_render(
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

    println!("Starting render...");

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
