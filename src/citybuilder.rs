//  citybuilder.rs -- draw a simple city.
//
//  Part of render-bench.
//
//  Used for generating simple 3D scenes for benchmarking purposes.
//
use anyhow::Error;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::sync::{Arc,Mutex};
use glam::{Mat3, Mat4, Vec2, UVec2, Vec3, Vec4, Quat};
use rend3::{
    types::{
        Mesh, MeshHandle, MeshBuilder, MaterialHandle,
        Texture, TextureHandle, TextureFormat, Object, ObjectHandle,
        },
    Renderer,
};

pub struct CityObject {
    object_handle: ObjectHandle,
}

pub struct CityState {
    pub desired_count: usize,                       // how many to create
    pub objects: Vec<CityObject>                    // the objects
}

impl CityState {
    /// Usual new
    pub fn new(desired_count: usize) -> CityState {
        CityState { desired_count, objects: Vec::new() }
    }
}

/// City Builder - a very simple procedural content generator.
//  Just enough to create something complicated to mimic the load of
//  rendering a few city blocks.
pub struct CityBuilder {
    pub threads: Vec<thread::JoinHandle<()>>,           // the threads 
    pub state: Arc<Mutex<CityState>>,                   // shared state
    pub stop_flag: Arc<AtomicBool>,                     // set to stop
}

impl CityBuilder {
    /// Create but do not start yet
    pub fn new(desired_count: usize) -> CityBuilder {
        CityBuilder {
            state: Arc::new(Mutex::new(CityState::new(desired_count))),
            threads: Vec::new(),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start and fire off threads.        
    pub fn start(&mut self, thread_count: usize, renderer: Arc<Renderer>) {
        assert!(thread_count < 100);                // sanity
        self.init();                                // any needed pre-thread init
        for n in 0..thread_count {
            let renderer_clone = Arc::clone(&renderer);
            let state_clone = Arc::clone(&self.state);
            let stop_clone = Arc::clone(&self.stop_flag);
            let handle = thread::spawn(move || {
                Self::run(state_clone, renderer_clone, n, stop_clone);
            });
            self.threads.push(handle);             // accumulate threads
        }   
    }
    
    /// Call to shut down
    pub fn stop(&mut self) {
        println!("Beginning shutdown of worker threads.");
        self.stop_flag.store(true, Ordering::Relaxed);     // other threads check this
        for item in self.threads.drain(..) {
            item.join().unwrap();
        }
        println!("All worker threads shut down.");       
    }
    
    /// Pre-spawn initialization
    fn init(&mut self) {
    }
    
    /// Actually does the work
    fn run(state: Arc<Mutex<CityState>>, renderer_clone: Arc<Renderer>, id: usize, stop_flag: Arc<AtomicBool>) {
        loop {
            if stop_flag.load(Ordering::Relaxed) { break }          // shut down
            std::thread::sleep(Duration::from_millis(10));          // ***TEMP TEST***
        }
    }
}			
