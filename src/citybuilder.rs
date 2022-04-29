//  citybuilder.rs -- draw a simple city.
//
//  Part of render-bench.
//
//  Used for generating simple 3D scenes for benchmarking purposes.
//
use super::solids;
use core::f32::consts::PI;
use glam::{Quat, Vec3};
use rend3::{
    types::{ObjectHandle, TextureHandle},
    Renderer,
};
use image::RgbaImage;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

//  Supplied parameters for building the city
#[derive(Debug, Clone)]
pub struct CityParams {
    building_count: usize,                        // number of buildings to generate
    texture_dir: String,                          // directory path to content
    texture_files: Vec<(String, String, String, f32)>, // texture name, albedo file, normal file, scale
}

impl CityParams {
    //  Params are (texture name, albedo file, normal file)
    pub fn new(
        building_count: usize,
        texture_dir: String,
        texture_files: Vec<(&str, &str, &str, f32)>,
    ) -> CityParams {
        CityParams {
            building_count,
            texture_dir,
            texture_files: texture_files
                .iter()
                .map(|item| (item.0.to_string(), item.1.to_string(), item.2.to_string(), item.3))
                .collect(),
        }
    }
}

pub struct CityObject {
    object_handle: ObjectHandle,
}

pub struct CityState {
    pub objects: Vec<CityObject>, // the objects
    pub textures: TextureSetRgbaMap,            // map of all the textures, as ImageRgba, not TextureHandle
}

impl CityState {
    /// Usual new
    pub fn new() -> CityState {
        CityState {
            objects: Vec::new(),
            textures: HashMap::new(),
        }
    }
}

/// City Builder - a very simple procedural content generator.
//  Just enough to create something complicated to mimic the load of
//  rendering a few city blocks.
pub struct CityBuilder {
    pub threads: Vec<thread::JoinHandle<()>>, // the threads
    pub state: Arc<Mutex<CityState>>,         // shared state
    pub stop_flag: Arc<AtomicBool>,           // set to stop
    pub params: CityParams,                   // params
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
        assert!(thread_count < 100); // sanity
        self.init(&renderer); // any needed pre-thread init
        for n in 0..thread_count {
            let renderer_clone = Arc::clone(&renderer);
            let state_clone = Arc::clone(&self.state);
            let stop_clone = Arc::clone(&self.stop_flag);
            let handle = thread::spawn(move || {
                Self::run(state_clone, renderer_clone, n, stop_clone);
            });
            self.threads.push(handle); // accumulate threads
        }
    }

    /// Call to shut down
    pub fn stop(&mut self) {
        println!("Beginning shutdown of worker threads.");
        self.stop_flag.store(true, Ordering::Relaxed); // other threads check this
        for item in self.threads.drain(..) {
            item.join().unwrap();
        }
        println!("All worker threads shut down.");
    }

    /// Load texture files. List of (texturename, albedo map, normal map)
    /// Files should be power of 2 and square, PNG format.
    /// Textures needed: "brick", "stone", "ground", "wood"
    fn load_texture(
        renderer: &Renderer,
        dir: &str,
        fileinfo: &(String, String, String),
    ) -> (String, (TextureHandle, TextureHandle)) {
        let (tex, albedo_name, normal_name) = fileinfo;
        let albedo_filename = format!("{}/{}", dir, albedo_name);
        let normal_filename = format!("{}/{}", dir, normal_name);
        (
            tex.clone(),
            (
                solids::create_simple_texture(renderer, &albedo_filename)
                    .unwrap_or_else(|err| panic!("Texture loading error: {:?}", err)),
                solids::create_simple_texture(renderer, &normal_filename)
                    .unwrap_or_else(|err| panic!("Texture loading error: {:?}", err))
            ),
        )
    }

    /// Pre-spawn initialization
    fn init(&mut self, renderer: &Renderer) {
        println!("Loading texture files.");
        //  Load all the textures
        self.state.lock().unwrap().textures =
            TextureSetRgba::new_map(&self.params.texture_dir, &self.params.texture_files);
        /*
        self.state.lock().unwrap().textures = self
            .params
            .texture_files
            .iter()
            .map(|item| Self::load_texture(renderer, &self.params.texture_dir, item))
            .collect();
        */
        println!("Content loaded.");
    }

    /// Actually does the work
    fn run(
        state: Arc<Mutex<CityState>>,
        renderer: Arc<Renderer>,
        id: usize,
        stop_flag: Arc<AtomicBool>,
    ) {
        //  Convert all the textures from RGBA to texture handles.
        let city_textures = CityTextures::new_from_map(&renderer, &state.lock().unwrap().textures);
        
        //  Make ground plane
        const WORLD_SIZE: f32 = 256.0; // one SL region size
        let _ground_handle = solids::create_simple_block(
            &renderer,
            Vec3::new(WORLD_SIZE, 0.5, WORLD_SIZE), // Ground object
            Vec3::ZERO,
            Vec3::new(0.0, -0.25, 0.0), // ground surface is at Z=0.0
            Quat::IDENTITY,             // no rotation
            &city_textures.ground,
        );
        //  ***TEMP TEST*** Make one brick block appear.
        let object_handle = solids::create_simple_block(
            &renderer,
            Vec3::new(1.5, 3.0, 0.2), // Brick wall
            Vec3::ZERO,
            Vec3::new(5.0, 1.5, 0.0),
            Quat::IDENTITY, // no rotation
            &city_textures.brick,
        );
        let new_city_object = CityObject { object_handle };
        state.lock().unwrap().objects.push(new_city_object); // keep around

        //  Make a wall section appear
        const WALL_WIDTH: f32 = 2.0;
        for i in 0..20 {
            let pos = Vec3::new(WALL_WIDTH * (i as f32), 0.0, 5.0);
            let object_handles = draw_wall_section(
                &renderer,
                WallKind::Window,
                Vec3::new(WALL_WIDTH, 3.0, 0.2), // Brick wall
                pos,
                Quat::IDENTITY, // no rotation
                &city_textures,
            );
            state
                .lock()
                .unwrap()
                .objects
                .extend(object_handles.iter().map(|object_handle| CityObject {
                    object_handle: object_handle.clone(),
                })); // keep objects around
        }

        //  One story of a building
        let wall_spec: (&[WallKind], &[WallKind]) = (
            &[
                WallKind::Door,
                WallKind::Window,
                WallKind::Solid,
                WallKind::Solid,
            ],
            &[WallKind::Window, WallKind::Solid],
        );
        let story_pos = Vec3::new(0.0, 0.0, 25.0);
        let story_object_handles = draw_one_story(
            &renderer,
            wall_spec,
            Vec3::new(WALL_WIDTH, 3.0, 0.2),
            story_pos,
            Quat::IDENTITY,
            &city_textures,
        );
        state
            .lock()
            .unwrap()
            .objects
            .extend(story_object_handles.iter().map(|object_handle| CityObject {
                object_handle: object_handle.clone(),
            })); // keep objects around
            
        //  Multiple one-story buildings
        const BLDG_ROWS: usize = 25;
        const BLDG_SPACING: f32 = 10.0;
        let bldg_initialpos = Vec3::new(-BLDG_SPACING*(BLDG_ROWS as f32)*0.5, 0.0, -BLDG_SPACING*(BLDG_ROWS as f32)*0.5); // center array
        for i in 0..BLDG_ROWS {
            for j in 0..BLDG_ROWS {
                let story_pos = Vec3::new((i as f32)*BLDG_SPACING, 0.0, (j as f32)*BLDG_SPACING) + bldg_initialpos;
                let story_object_handles = draw_one_story(
                    &renderer,
                    wall_spec,
                    Vec3::new(WALL_WIDTH, 3.0, 0.2),
                    story_pos,
                    Quat::IDENTITY,
                    &city_textures,
                );
                state
                    .lock()
                    .unwrap()
                    .objects
                    .extend(story_object_handles.iter().map(|object_handle| CityObject {
                     object_handle: object_handle.clone(),
                    })); // keep objects around
            
            }
        };
                 //  ***END TEMP***
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            } // shut down
            std::thread::sleep(Duration::from_millis(10)); // ***TEMP TEST***
        }
    }
}

//
//  WallKind
//
#[derive(Debug, Copy, Clone)]
enum WallKind {
    None,
    Solid,
    Door,
    Window,
}

/// Building textures
pub struct TextureSetRgba {
    albedo: RgbaImage,          // albedo image
    normal: RgbaImage,          // normal image
    texture_scale: f32,
}

type TextureSetRgbaMap = HashMap<String, TextureSetRgba>;

impl TextureSetRgba {
    //  Make a map with all the textures as Rgba images.
    pub fn new_map(dir: &str, textures: &Vec<(String, String, String, f32)>) -> TextureSetRgbaMap {
    //  Read textures, save all RGBAs
        let mut output = HashMap::new();
        for (name, albedo_filename, normal_filename, texture_scale) in textures {    
            let texture_set = TextureSetRgba {
                albedo: solids::read_texture(format!("{}/{}", dir, albedo_filename).as_str()).unwrap(),
                normal: solids::read_texture(format!("{}/{}", dir, normal_filename).as_str()).unwrap(),
                texture_scale: *texture_scale
            };
            output.insert(name.clone(), texture_set);
        }
        output
    }
}
pub type TextureSet = (TextureHandle, TextureHandle, f32);    // albedo, normal, scale
/// The textures we need for our little city.
pub struct CityTextures {
    stone: TextureSet,      // used for columns
    brick: TextureSet,      // used for walls
    floor: TextureSet,      // used for floors
    ceiling: TextureSet,    // used for ceilings
    roof: TextureSet,       // used for roofs
    ground: TextureSet,     // used for ground
}

impl CityTextures {
    //  Make a new set of textures from an Rgba.
    //  This duplicates the actual bitmaps, on purpose, to increase texture usage for load testing.   
    pub fn new_from_map(renderer: &Renderer, rgbas: &TextureSetRgbaMap) -> CityTextures {
        let make_textures = |label: &str, item: &TextureSetRgba| (
            solids::create_texture_from_rgba(renderer, label, &item.albedo),
            solids::create_texture_from_rgba(renderer, label, &item.normal),
            item.texture_scale);
        let get_textures = |key| make_textures(key, rgbas.get(key).unwrap());
        CityTextures {
            stone: get_textures("stone"),
            brick: get_textures("brick"),
            floor: get_textures("floor"),
            ceiling: get_textures("ceiling"),
            roof: get_textures("roof"),
            ground: get_textures("roof")
        }
    }
}
//
//  Draw functions for various objects
//

/// Draw one story of a building.
//  A story is a rectangular set of wall sections.
//  Specify door, window, solid sections.
//  Specify two sides; the other side is mirrored.
//
fn draw_one_story(
    renderer: &Renderer,
    wall_spec: (&[WallKind], &[WallKind]),
    size: Vec3,
    pos: Vec3,
    rot: Quat,
    textures: &CityTextures,
) -> Vec<ObjectHandle> {
    let width = size[0];
    let height = size[1];
    let thickness = size[2];
    let (front, side) = wall_spec;
    let front_width = (front.len() as f32) * width;
    let side_width = (side.len() as f32) * width;
    let mut objects = Vec::new();
    //  Draw each face, given offsets from input position
    let draw_one_face = |startpos, itemoffset, itemrot, kind: &WallKind| {
        draw_wall_section(
            renderer,
            *kind,
            size,
            startpos + (itemrot * rot) * itemoffset,
            itemrot * rot,
            textures,
        )
    };
    //  Front
    objects.extend(
        front
            .iter()
            .enumerate()
            .flat_map(|(i, kind)| {
                let itemoffset = Vec3::new((i as f32) * width, 0.0, 0.0);
                let startpos = pos;
                draw_one_face(startpos, itemoffset, Quat::IDENTITY, kind)
            })
            .collect::<Vec<ObjectHandle>>(),
    );
    //  Right side
    objects.extend(
        side.iter()
            .enumerate()
            .flat_map(|(i, kind)| {
                let itemoffset = Vec3::new((i as f32) * width, 0.0, 0.0); // per item offset
                let startpos = pos + rot * Vec3::new(front_width, 0.0, 0.0);
                draw_one_face(startpos, itemoffset, Quat::from_rotation_y(-PI * 0.5), kind)
            })
            .collect::<Vec<ObjectHandle>>(),
    );
    //  Back
    objects.extend(
        front
            .iter()
            .enumerate()
            .flat_map(|(i, kind)| {
                let itemoffset = Vec3::new((i as f32) * width, 0.0, 0.0); // per item offset
                let startpos = pos + rot * Vec3::new(front_width, 0.0, side_width);
                draw_one_face(startpos, itemoffset, Quat::from_rotation_y(-PI), kind)
            })
            .collect::<Vec<ObjectHandle>>(),
    );
    //  Left side
    objects.extend(
        side.iter()
            .enumerate()
            .flat_map(|(i, kind)| {
                let itemoffset = Vec3::new((i as f32) * width, 0.0, 0.0); // per item offset
                let startpos = pos + rot * Vec3::new(0.0, 0.0, side_width);
                draw_one_face(startpos, itemoffset, Quat::from_rotation_y(-PI * 1.5), kind)
            })
            .collect::<Vec<ObjectHandle>>(),
    );
    //  Floor and ceiling
    let floor_size = Vec3::new(front_width, 0.1, side_width);
    objects.extend(draw_floor_and_ceiling(renderer, height, floor_size, pos, rot, textures));
    objects.extend(draw_roof(renderer, height, thickness, floor_size, pos, rot, textures));
    objects
}

/// Draw a wall section.
//  A wall section has a column at the left.
//  A row of these in the X direction makes a wall.
//  Origin of the wall section is at the base of the column.
fn draw_wall_section(
    renderer: &Renderer,
    wall_kind: WallKind,
    size: Vec3,
    pos: Vec3,
    rot: Quat,
    textures: &CityTextures
) -> Vec<ObjectHandle> {
    //  Precompute wall info
    let width = size[0];
    let thickness = size[2];
    let height = size[1];
    let column_thickness = thickness * 2.0;
    let wall_width = width - column_thickness;
    //  Draw column. Base of column is atop pos.
    let mut objects = vec![solids::create_simple_block(
        renderer,
        Vec3::new(column_thickness, height, column_thickness), // size of column
        Vec3::new(0.0, height / 2.0, 0.0),                     // base at zero
        pos,
        rot,
        &textures.stone,
    )];
    // Draw wall section
    match wall_kind {
        WallKind::None => {
            // Open section, no wall, just a column
        }
        WallKind::Solid => {
            //  Solid wall section
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, height, thickness), // size of column
                Vec3::new((column_thickness + wall_width) / 2.0, height / 2.0, 0.0), // base at zero
                pos,
                rot,
                &textures.brick,
            ));
        }
        WallKind::Door => {
            //  Door. Open except for top part.
            let opening_height = height * 0.75; // height of door opening
            let top_height = height - opening_height;
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, top_height, thickness), // size of door lintel
                Vec3::new(
                    (column_thickness + wall_width) / 2.0,
                    opening_height + top_height / 2.0,
                    0.0,
                ), // base at zero
                pos,
                rot,
                &textures.brick,
            ));
        }
        WallKind::Window => {
            //  Window. Open at vertical center.
            let opening_height = height * 0.5; // height of window
            let top_height = height * 0.25;
            let bottom_height = height - opening_height - top_height;
            //  Top part
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, top_height, thickness), // size of window top
                Vec3::new(
                    (column_thickness + wall_width) / 2.0,
                    bottom_height + opening_height + top_height / 2.0,
                    0.0,
                ), // base at zero
                pos,
                rot,
                &textures.brick,
            ));
            //  Bottom part
            objects.push(solids::create_simple_block(
                renderer,
                Vec3::new(wall_width, bottom_height, thickness), // size of window bottom
                Vec3::new(
                    (column_thickness + wall_width) / 2.0,
                    bottom_height / 2.0,
                    0.0,
                ), // base at zero
                pos,
                rot,
                &textures.brick,
            ));
        }
    }
    objects // return handles which keep objects alive
}

/// Draw a floor section
//  Pos is the same as for a story, the lower left hand corner.
//  Floor texture on top, ceiling texture on bottom.
fn draw_floor_and_ceiling(
    renderer: &Renderer,
    height: f32,                // floor height
    size: Vec3,
    pos: Vec3,
    rot: Quat,
    textures: &CityTextures,
) -> Vec<ObjectHandle> {
    let thickness = size[1];            // thickness of floor
    let center = size*0.5;              // center of block relative to pos
    vec![
    solids::create_simple_block(        // floor
        renderer,
        size,
        center + Vec3::new(0.0, - thickness*0.45, 0.0),
        pos,
        rot,
        &textures.floor,
    ),
    solids::create_simple_block(        // ceiling
        renderer,
        size,
        center + Vec3::new(0.0, height - thickness*0.55, 0.0),
        pos,
        rot,
        &textures.ceiling,
    ),
    ]
}
//  Pos is the same as for a story, the lower left hand corner.
//  Floor texture on top, ceiling texture on bottom.
fn draw_roof(
    renderer: &Renderer,
    height: f32,                // floor height
    thickness: f32,             // of parapet, not roof
    size: Vec3,
    pos: Vec3,
    rot: Quat,
    textures: &CityTextures,
) -> Vec<ObjectHandle> {
    let center = size*0.5 + Vec3::new(0.0, height, 0.0);
    vec![
    solids::create_simple_block(        // roof
        renderer,
        Vec3::new(size[0]+thickness, thickness*0.5, size[2]+thickness),   // thin roof so as not to clash with parapet
        center,
        pos,
        rot,
        &textures.roof,
    ),
    solids::create_simple_block(        // front
        renderer,
        Vec3::new(size[0]+thickness*3.0, thickness, thickness), // strip along front
        center - Vec3::new(0.0, 0.0, (size[2]+2.0*thickness)*0.5), // center pos
        pos,
        rot,
        &textures.stone,
    ),
    solids::create_simple_block(        // back
        renderer,
        Vec3::new(size[0]+thickness*3.0, thickness, thickness), // strip along back
        center - Vec3::new(0.0, 0.0, -(size[2]+2.0*thickness)*0.5), // center pos
        pos,
        rot,
        &textures.stone,
    ),
    solids::create_simple_block(        // left side
        renderer,
        Vec3::new(thickness, thickness, size[2]+thickness), // strip along left side
        center - Vec3::new((size[0]+2.0*thickness)*0.5, 0.0, 0.0), // center pos
        pos,
        rot,
        &textures.stone,
    ),
    solids::create_simple_block(        // left side
        renderer,
        Vec3::new(thickness, thickness, size[2]+thickness), // strip along left side
        center - Vec3::new(-(size[0]+2.0*thickness)*0.5, 0.0, 0.0), // center pos
        pos,
        rot,
        &textures.stone,
    ),
    ]
}
