//  solids.rs -- draw simple solid objects
//
//  Part of render-bench.
//
//  Used for generating simple 3D scenes for benchmarking purposes.
//
use anyhow::Error;
use glam::{Mat3, Mat4, Quat, UVec2, Vec2, Vec3, Vec4};
use rend3::{
    types::{
        MaterialHandle, Mesh, MeshBuilder, Object, ObjectHandle, Texture, TextureFormat,
        TextureHandle,
    },
    Renderer,
};

use core::num::NonZeroU32;
use rend3_routine::pbr::{AlbedoComponent, NormalTexture, PbrMaterial};

/// Create a simple block.
//  Each block gets its own material, because we do it that way in the SL viewer.
//  No instancing here.
pub fn create_simple_block(
    renderer: &Renderer,
    scale: Vec3,                                         // this rescales the actual mesh
    offset: Vec3,                                        // this offsets the coords in the mesh
    pos: Vec3,                                           // position in transform
    rot: Quat,                                           // rotation
    texture_info: (&TextureHandle, &TextureHandle, f32), // (albedo, normal, scale)
) -> ObjectHandle {
    let (albedo_handle, normal_handle, texture_scale) = texture_info; // unpack tuple
    ////println!("Add built-in object at {:?} size {:?}", pos, scale); // ***TEMP***
    let material = create_simple_material(renderer, albedo_handle, normal_handle); // the texture
    let mesh = create_mesh(scale, offset, texture_scale);
    let mesh_handle = renderer.add_mesh(mesh);
    //  Add object to Rend3 system
    renderer.add_object(Object {
        mesh_kind: rend3::types::ObjectMeshKind::Static(mesh_handle),
        material,
        transform: Mat4::from_scale_rotation_translation(Vec3::ONE, rot, pos),
    })
}

/// Very simple texture, but a bit of shinyness.
pub fn create_simple_material(
    renderer: &Renderer,
    albedo_handle: &TextureHandle,
    normal_handle: &TextureHandle,
) -> MaterialHandle {
    let diffuse_color = Vec4::ONE; // white
                                   //  Albedo from texture
    let albedo = AlbedoComponent::TextureValue {
        texture: albedo_handle.clone(),
        value: diffuse_color,
    };
    let normal = NormalTexture::Tricomponent(normal_handle.clone(), Default::default());
    let pbr_material = PbrMaterial {
        albedo,
        normal,
        ////aomr_textures,
        ao_factor: Some(1.0),
        metallic_factor: Some(0.2),
        roughness_factor: Some(0.2), // ***TEMP TEST***
        uv_transform0: Mat3::IDENTITY,
        uv_transform1: Mat3::IDENTITY, // not used yet
        ..Default::default()
    };
    renderer.add_material(pbr_material) // add material to Rend3 system
}

/// Create a simple texture for display. No normalization, etc.
pub fn create_simple_texture(renderer: &Renderer, file_name: &str) -> Result<TextureHandle, Error> {
    //  Read from file.
    let img = image::io::Reader::open(file_name)?.decode()?;
    let rgba = img.to_rgba8(); // to desired format
                               //  Convert to Rend3 format.
    let mips = 1; // no mipmapping for now
    let texture = Texture {
        label: Some(file_name.to_string()),
        format: TextureFormat::Rgba8UnormSrgb, // per WGPU tutorial
        size: UVec2::new(rgba.width(), rgba.height()),
        data: rgba.into_raw(),
        //// TODO: automatic mipmapping (#53)
        mip_count: rend3::types::MipmapCount::Specific(NonZeroU32::new(mips).unwrap()),
        mip_source: rend3::types::MipmapSource::Uploaded,
    };
    Ok(renderer.add_texture_2d(texture)) // put into GPU
}

//  Create a mesh object with the appropriate scale and origin offset.
pub fn create_mesh(scale: Vec3, offset: Vec3, texture_scale: f32) -> Mesh {
    let mul_elements = |a: Vec3, b: Vec3| Vec3::new(a[0] * b[0], a[1] * b[1], a[2] * b[2]); // why is this never built into vec libraries?
                                                                                            //  Scale and offset verts.
    let vertex_positions: Vec<Vec3> = UNIT_CUBE_VERTS
        .iter()
        .map(|v| mul_elements(scale, (*v).into()) + offset)
        .collect();
    let normals: Vec<Vec3> = UNIT_CUBE_FACE_NORMALS.iter().map(|v| (*v).into()).collect();
    let uvs = calc_uvs(&vertex_positions, &normals, texture_scale);
    //  Create UVs.
    MeshBuilder::new(vertex_positions.to_vec(), rend3::types::Handedness::Left)
        .with_indices(UNIT_CUBE_INDICES.to_vec())
        .with_vertex_normals(normals)
        .with_vertex_uv0(uvs)
        .build()
        .unwrap()
}

/// Dominant axis from normal. Just the longest direction.
fn norm_to_axis(normal: &Vec3) -> u8 {
    if normal[0].abs() > normal[1].abs() && normal[0].abs() > normal[2].abs() {
        0 // X wins
    } else if normal[1].abs() > normal[2].abs() {
        1 // Y wins
    } else {
        2 // Z wins
    }
}

///  Calculate planar UVs. This has to agree with how SL does it.
fn calc_uv(axis: u8, vertex: &Vec3, normal: &Vec3) -> Vec2 {
    match axis {
        0 => calc_single_uv(Vec2::new(vertex[2], vertex[1]), normal[0]), // X normal wins, use Y and Z
        1 => calc_single_uv(Vec2::new(vertex[0], vertex[2]), normal[1]), // Y normal wins, use X and Z
        2 => calc_single_uv(Vec2::new(vertex[0], vertex[1]), -normal[2]), // Z normal wins, use X and Y, invert
        _ => panic!("calc_planar_uv - axis invalid"),                     // no way
    }
}

/// Calculate one UV value
fn calc_single_uv(pos: Vec2, normal: f32) -> Vec2 {
    //  Bounds are normally -0.5 .. 0.5, but they do not have to be.
    //  We must rescale into that range.

    let u = pos[0];
    let v = pos[1]; // UVs
    const MESH_UV_SCALE_FACTOR: f32 = 2.0; // rescale vertex space to UV space
    const MESH_UV_OFFSET: f32 = 0.25; // offset because UVs are 0..1
    let sign = |val: f32| if val.is_sign_negative() { -1.0 } else { 1.0 };
    Vec2::new(u * sign(normal) + MESH_UV_OFFSET, v + MESH_UV_OFFSET) * MESH_UV_SCALE_FACTOR
}

/// Default UVs, scaled as mesh is scaled, so repetitive textures work.
//  So this is a planar mapping. We can use it for bricks and such.
fn calc_uvs(vertex_positions: &[Vec3], normals: &[Vec3], texture_scale: f32) -> Vec<Vec2> {
    vertex_positions
        .iter()
        .zip(normals)
        .map(|(v, n)| calc_uv(norm_to_axis(n), v, n) * texture_scale)
        .collect()
}

//  The unit cube. No shared verts at corners.
const UNIT_CUBE_VERTS: [[f32; 3]; 24] = [
    // far side (0.0, 0.0, 1.0)
    [-0.5, -0.5, 0.5],
    [0.5, -0.5, 0.5],
    [0.5, 0.5, 0.5],
    [-0.5, 0.5, 0.5],
    // near side (0.0, 0.0, -1.0)
    [-0.5, 0.5, -0.5],
    [0.5, 0.5, -0.5],
    [0.5, -0.5, -0.5],
    [-0.5, -0.5, -0.5],
    // right side (1.0, 0.0, 0.0)
    [0.5, -0.5, -0.5],
    [0.5, 0.5, -0.5],
    [0.5, 0.5, 0.5],
    [0.5, -0.5, 0.5],
    // left side (-1.0, 0.0, 0.0)
    [-0.5, -0.5, 0.5],
    [-0.5, 0.5, 0.5],
    [-0.5, 0.5, -0.5],
    [-0.5, -0.5, -0.5],
    // top (0.0, 1.0, 0.0)
    [0.5, 0.5, -0.5],
    [-0.5, 0.5, -0.5],
    [-0.5, 0.5, 0.5],
    [0.5, 0.5, 0.5],
    // bottom (0.0, -1.0, 0.0)
    [0.5, -0.5, 0.5],
    [-0.5, -0.5, 0.5],
    [-0.5, -0.5, -0.5],
    [0.5, -0.5, -0.5],
];

//  The usual face normals.
const UNIT_CUBE_FACE_NORMALS: [[f32; 3]; 24] = [
    [0.0, 0.0, 1.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
    [0.0, 0.0, -1.0],
    [0.0, 0.0, -1.0],
    [0.0, 0.0, -1.0],
    [1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, -1.0, 0.0],
];

//  The triangles, 12 of them.
const UNIT_CUBE_INDICES: [u32; 36] = [
    0, 1, 2, 2, 3, 0, // far
    4, 5, 6, 6, 7, 4, // near
    8, 9, 10, 10, 11, 8, // right
    12, 13, 14, 14, 15, 12, // left
    16, 17, 18, 18, 19, 16, // top
    20, 21, 22, 22, 23, 20, // bottom
];

/*

    rend3::types::MeshBuilder::new(vertex_positions.to_vec(), rend3::types::Handedness::Left)
        .with_indices(index_data.to_vec())
        .build()
        .unwrap()
}
*/
