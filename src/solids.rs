//  solids.rs -- draw simple solid objects
//
//  Used for generating simple 3D scenes for benchmarking purposes.
//
use glam::{DVec2, Mat3A, Mat4, UVec2, Vec3, Vec3A, Quat};			
use rend3::{
    types::{
        Backend, Camera, Mesh, MeshHandle, Material, MaterialHandle,
        Texture, TextureHandle, TextureFormat, Object, ObjectHandle
    },
    util::typedefs::FastHashMap,
    Renderer, RendererProfile,
};





/// Create one object at given coordinates
fn create_block(
    renderer: &Renderer,
    mesh: MeshHandle,
    material: MaterialHandle,
    scale: Vec3,
    pos: Vec3,
    rot: Quat,
) -> ObjectHandle {
    println!("Add built-in object at {:?} size {:?}", pos, scale); // ***TEMP***
    renderer.add_object(Object {
        ////mesh: mesh.expect("Built-in object mesh invalid"),
        mesh_kind: rend3::types::ObjectMeshKind::Static(mesh),
        material,
        transform: Mat4::from_scale_rotation_translation(scale, rot, pos),
    })
}

//  Create a mesh object with the appropriate scale and origin offset.
pub fn create_mesh(scale: Vec3, offset: Vec3, texture_scale: f32, planar_mapping: bool) { //// 	-> Mesh {
    
    let mul_elements = |a: Vec3, b: Vec3| Vec3::new(a[0]*b[0], a[1]*b[1], a[2]*b[2]);    // why is this never built into vec libraries?
    let vertex_positions: Vec<Vec3> = UNIT_CUBE_VERTS.iter().map(|v| mul_elements(scale, (*v).into()) + offset).collect();
    //  Create UVs.
    let vertex_uvs = if planar_mapping { 
        panic!("Planar mapping unimplemented"); // no planar texture mapping yet
    } else {
        1
    };
}

//  Calculate normals from triangles. Works for any mesh. Averages normals at shared vertices, if any.
pub fn calc_normals(vertex_positions: &Vec<Vec3>, index_data: &Vec<i32>) -> Vec<Vec3> {
    let mut normals: Vec<Vec3> = (0..vertex_positions.len()).map(|_| Vec3::new(0.0, 0.0, 0.0)).collect(); // Init normals
    let tri_cross_from_pts = |v0: Vec3, v1: Vec3, v2: Vec3| (v1-v0).cross(v2-v0);   // normal of triangle
    let tri_vertex = |n| vertex_positions[index_data[n as usize] as usize]; // vertex of triangle
    for i in (0..index_data.len()).step_by(3) {   // iterate over triangles
        let cross_product = tri_cross_from_pts(tri_vertex(i), tri_vertex(i+1), tri_vertex(i+2));         
        let normal = cross_product.normalize();        // usual cross product appraoch
        for j in i..i+3 {                               // add to relevant verts
            normals[index_data[j] as usize] = normal;
        }
    }
    for mut item in &mut normals { *item = item.normalize() }            // final normalize
    normals

}

//  The unit cube. No shared verts at corners.
const UNIT_CUBE_VERTS: [[f32;3];24] = [
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
/*
const UNIT_CUBE_FACE_NORMALS: [Vec3;24] = [
    Vec3::Z, Vec3::Z, Vec3::Z, Vec3::Z,
    -Vec3::Z, -Vec3::Z, -Vec3::Z, -Vec3::Z,
     Vec3::X, Vec3::X, Vec3::X, Vec3::X,
    -Vec3::X, -Vec3::X, -Vec3::X, -Vec3::X,
     Vec3::Y, Vec3::Y, Vec3::Y, Vec3::Y,
    -Vec3::Y, -Vec3::Y, -Vec3::Y, -Vec3::Y
    ];
*/    
    

//  The triangles, 12 of them.
const UNIT_CUBE_INDICES: [u32;36] = [
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

