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
use glam::{Vec3, Quat};
use rend3::{
    types::{
        TextureHandle, ObjectHandle,
        },
    Renderer,
};

//  Supplied parameters for building the city
#[derive(Debug, Clone)]
pub struct CityParams {
    building_count: usize,                          // number of buildings to generate
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
            params: city_params,
            
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
        let normal_filename = format!("{}/{}", dir, normal_name);
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
    fn run(state: Arc<Mutex<CityState>>, renderer: Arc<Renderer>, id: usize, stop_flag: Arc<AtomicBool>) {
        let brick_textures = state.lock().unwrap().textures.get("brick").unwrap().clone();    // get brick textures
        let ground_textures = state.lock().unwrap().textures.get("ground").unwrap().clone();    // get brick textures
        //  Make ground plane
        const WORLD_SIZE: f32 = 256.0;                      // one SL region size
        let _ground_handle = solids::create_simple_block(
            &renderer,
            Vec3::new(WORLD_SIZE, 0.5, WORLD_SIZE),          // Ground object
            Vec3::ZERO,
            Vec3::new(0.0, -0.25, 0.0), // ground surface is at Z=0.0
            Quat::IDENTITY,             // no rotation
            (&ground_textures.0, &ground_textures.1, 0.25));
        //  ***TEMP TEST*** Make one brick block appear.
        let object_handle = solids::create_simple_block(
            &renderer,
            Vec3::new(1.5, 3.0, 0.2),          // Brick wall
            Vec3::ZERO,
            Vec3::new(5.0, 1.5, 0.0),
            Quat::IDENTITY,             // no rotation
            (&brick_textures.0,
            &brick_textures.1,
            0.25).clone());
        let new_city_object = CityObject{ object_handle };
        state.lock().unwrap().objects.push(new_city_object);          // keep around
        
        //  Make a wall section appear
        const WALL_WIDTH: f32 = 2.0;
        for i in 0..20 {
            let pos = Vec3::new(WALL_WIDTH*(i as f32), 0.0, 5.0);
            let object_handles = draw_wall_section(
                &renderer,
                WallKind::Window,
                Vec3::new(WALL_WIDTH, 3.0, 0.2),          // Brick wall
                pos,
                Quat::IDENTITY,             // no rotation
                (&brick_textures.0, &brick_textures.1, 0.25)
            );
            state.lock().unwrap().objects
                .extend(object_handles
                .iter()
                .map(|object_handle| CityObject { object_handle: object_handle.clone() }));   // keep objects around
            
        }           
        //  ***END TEMP***
        loop {
            if stop_flag.load(Ordering::Relaxed) { break }          // shut down
            std::thread::sleep(Duration::from_millis(10));          // ***TEMP TEST***
        }
    }
}

//
//  WallKind
//
enum WallKind {
    None,
    Solid,
    Door,
    Window
}
//
//  Draw functions for various objects
//
/// Draw a wall section.
//  A wall section has a column at the left.
//  A row of these in the X direction makes a wall.
//  Origin of the wall section is at the base of the column.
fn draw_wall_section(renderer: &Renderer, wall_kind: WallKind, size: Vec3, pos: Vec3, rot: Quat, 
        textures: (&TextureHandle, &TextureHandle, f32)) -> Vec<ObjectHandle> {
    //  Precompute wall info
    let width = size[0];
    let thickness = size[2];
    let height = size[1];
    let column_thickness = thickness*2.0;
    let wall_width = width-column_thickness;
    let mut objects = Vec::new();
    //  Draw column. Base of column is atop pos.
    objects.push(solids::create_simple_block(
        renderer,
        Vec3::new(column_thickness, height, column_thickness),  // size of column
        Vec3::new(0.0, height/2.0, 0.0),    // base at zero
        pos,
        rot,
        textures.clone()));
    // Draw wall section
    match wall_kind {
        WallKind::None => {
            // Open section, no wall, just a column
        }                       
        WallKind::Solid => {
            //  Solid wall section
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, height, thickness),  // size of column
                Vec3::new(column_thickness + wall_width/2.0, height/2.0, 0.0),    // base at zero
                pos,
                rot,
                textures));
        }
        WallKind::Door => {
            //  Door. Open except for top part.
            let opening_height = height * 0.75;                    // height of door opening
            let top_height = height - opening_height;
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, top_height, thickness),  // size of door lintel
                Vec3::new(column_thickness + wall_width/2.0, opening_height + top_height/2.0, 0.0),    // base at zero
                pos,
                rot,
                textures));
        }
        WallKind::Window => {
            //  Window. Open at vertical center.
            let opening_height = height*0.5;                         // height of window
            let top_height = height*0.25;
            let bottom_height = height - opening_height - top_height;
            //  Top part
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, top_height, thickness),  // size of window top
                Vec3::new((column_thickness + wall_width)/2.0, bottom_height + opening_height + top_height/2.0, 0.0),    // base at zero
                pos,
                rot,
                textures));
            //  Bottom part
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, bottom_height, thickness),  // size of window bottom
                Vec3::new((column_thickness + wall_width)/2.0, bottom_height/2.0, 0.0),    // base at zero
                pos,
                rot,
                textures));
        }                      
    }
    objects                                 // return handles which keep objects alive
}	
