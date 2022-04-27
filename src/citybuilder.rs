//  citybuilder.rs -- draw a simple city.
//
//  Part of render-bench.
//
//  Used for generating simple 3D scenes for benchmarking purposes.
//
use super::solids;
use std::collections::HashMap;
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

//  Supplied parameters for building the city
#[derive(Debug, Clone)]
pub struct CityParams {
    building_count: usize,
    texture_dir: String,                            // directory path to content
    texture_files: Vec<(String, String, String)>,   // texture name, albedo file, normal file
}

impl CityParams {
    //  Params are (texture name, albedo file, normal file)
    pub fn new(building_count: usize, texture_dir: String, texture_files: Vec<(&str,&str,&str)>) -> CityParams {
        CityParams {
            building_count,
            texture_dir,
            texture_files: texture_files.iter().map(|item| (item.0.to_string(), item.1.to_string(), item.2.to_string())).collect(),
        }
    } 
}

pub struct CityObject {
    object_handle: ObjectHandle,
}

pub struct CityState {
    pub objects: Vec<CityObject>,                   // the objects
    pub textures: HashMap<String, (TextureHandle, TextureHandle)>    // the textures
}

impl CityState {
    /// Usual new
    pub fn new() -> CityState {
        CityState { objects: Vec::new(), textures: HashMap::new() }
    }
}

/// City Builder - a very simple procedural content generator.
//  Just enough to create something complicated to mimic the load of
//  rendering a few city blocks.
pub struct CityBuilder {
    pub threads: Vec<thread::JoinHandle<()>>,           // the threads 
    pub state: Arc<Mutex<CityState>>,                   // shared state
    pub stop_flag: Arc<AtomicBool>,                     // set to stop
    pub params: CityParams,                             // params
}

impl CityBuilder {
    /// Create but do not start yet
    pub fn new(city_params: CityParams) -> CityBuilder {
        CityBuilder {
            state: Arc::new(Mutex::new(CityState::new())),
            threads: Vec::new(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            params: city_params.clone(),
            
        }
    }
    
    /// Start and fire off threads.        
    pub fn start(&mut self, thread_count: usize, renderer: Arc<Renderer>) {
        assert!(thread_count < 100);                // sanity
        self.init(&renderer);                       // any needed pre-thread init
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
    
    /// Load texture files. List of (texturename, albedo map, normal map)
    /// Files should be power of 2 and square, PNG format.
    /// Textures needed: "brick", "stone", "ground", "wood"
    fn load_texture(renderer: &Renderer, dir: &str, fileinfo: &(String, String, String)) -> (String, (TextureHandle, TextureHandle)) {
        let (tex, albedo_name, normal_name) = fileinfo;
        let albedo_filename = format!("{}/{}", dir, albedo_name);
        let normal_filename = format!("{}/{}", dir, albedo_name);
        (tex.clone(),(solids::create_simple_texture(renderer, &albedo_filename).unwrap(),
        solids::create_simple_texture(renderer, &normal_filename).unwrap()))
    }

    
    /// Pre-spawn initialization
    fn init(&mut self, renderer: &Renderer) {
        //  Load all the textures
        self.state.lock().unwrap().textures = 
            self.params.texture_files.iter().map(|item| 
                Self::load_texture(renderer, &self.params.texture_dir, item)).collect();
    }
    
    /// Actually does the work
    fn run(state: Arc<Mutex<CityState>>, renderer_clone: Arc<Renderer>, id: usize, stop_flag: Arc<AtomicBool>) {
        loop {
            if stop_flag.load(Ordering::Relaxed) { break }          // shut down
            std::thread::sleep(Duration::from_millis(10));          // ***TEMP TEST***
        }
    }
}			
