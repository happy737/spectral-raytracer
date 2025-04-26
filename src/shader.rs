use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use nalgebra::{point, vector, Const, Matrix3, OMatrix, OPoint, Point3, Vector3};
use crate::{UICamera, UILight, UIObject, UIObjectType};

pub(crate) const F32_DELTA: f32 = 0.00001;
const NEW_RAY_MAX_BOUNCES: u32 = 30;

/// The position of the pixel on the screen. (0, 0) is the top left. 
#[derive(Copy, Clone)]
pub struct PixelPos {
    pub x: u32,
    pub y: u32,
}

/// The struct holds the width and height of the rendered frame. 
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

/// The struct holds the uniform data which is constant per frame. This includes things as the 
/// information about light sources or objects in the scene. 
#[derive(Clone)]
pub struct RaytracingUniforms {
    pub(crate) aabbs: Arc<Vec<Aabb>>,
    pub(crate) lights: Arc<Vec<Light>>,
    pub(crate) camera: Camera,
    pub(crate) frame_id: u32,
}

/// The struct representing the ray that is shot through the scene. It contains information about
/// the origin and direction as well as returned information such as color (intensity). 
struct Ray {
    origin: Point3<f32>,
    direction: Vector3<f32>,
    hit: bool,
    intensity: f32,
    skip_hit_shader: bool,
    max_bounces: u32,
    original_pixel_pos: PixelPos,
    hit_distance: f32,
}
impl Ray {
    /// Creates a new standard Ray with default values for the values which will be written to in 
    /// the shaders. 
    fn new(origin: Point3<f32>, direction: Vector3<f32>, max_bounces: u32, 
           original_pixel_pos: PixelPos) -> Ray {
        Ray {
            origin,
            direction: direction.normalize(),
            hit: false,
            intensity: 0.0,
            skip_hit_shader: false,
            max_bounces,
            original_pixel_pos,
            hit_distance: 0.0,
        }
    }
    
    /// Creates a new shadow ray. Shadow rays are rays which terminate upon hitting anything and 
    /// can thus be used to determine if an unobstructed line to another point exists. The 
    /// closest-hit shader will not be executed for this ray. The field hit will be set to true if 
    /// anything is hit. 
    fn new_shadow_ray(origin: Point3<f32>, direction: Vector3<f32>) -> Ray {
        Ray {
            origin, 
            direction,
            hit: false,
            intensity: 0.0,
            skip_hit_shader: true,
            max_bounces: 1, //technically unnecessary
            original_pixel_pos: PixelPos {x:0, y:0},    //dummy value
            hit_distance: 0.0,
        }
    }
}

/// AABBs (Axis Aligned Bounding Box) are structures defined by their smallest and largest Point of 
/// a cuboid. These structs hold an Enum which differentiates their content, for example a sphere 
/// (AABBType::Sphere) can be mathematically defined by its center and radius, both of which can be 
/// calculated from the two given points of the AABB. 
pub(crate) struct Aabb {
    min: Point3<f32>,
    max: Point3<f32>,
    aabb_type: AABBType,
}
impl Aabb {
    pub fn new_sphere(center: &Point3<f32>, radius: f32) -> Aabb {
        Aabb {
            min: point![center.x - radius, center.y - radius, center.z - radius],
            max: point![center.x + radius, center.y + radius, center.z + radius],
            aabb_type: AABBType::Sphere,
        }
    }
    
    pub fn new_box(center: &Point3<f32>, x_length: f32, y_length: f32, z_length: f32) -> Aabb {
        let x_half = x_length / 2.0;
        let y_half = y_length / 2.0;
        let z_half = z_length / 2.0;
        Aabb {
            min: point![center.x - x_half, center.y - y_half, center.z - z_half],
            max: point![center.x + x_half, center.y + y_half, center.z + z_half],
            aabb_type: AABBType::PlainBox,
        }
    }
}
enum AABBType {
    PlainBox,
    Sphere,
}

impl From<&UIObject> for Aabb {
    fn from(value: &UIObject) -> Self {
        let pos = point![value.pos_x, value.pos_y, value.pos_z];
        match value.ui_object_type {
            UIObjectType::PlainBox(x_length, y_length, z_length) => {
                Aabb::new_box(&pos, x_length, y_length, z_length)
            }
            UIObjectType::Sphere(radius) => {
                Aabb::new_sphere(&pos, radius)
            }
        }
    }
}

pub (crate) struct Light {
    position: Point3<f32>,
    intensity: f32,
}
impl Light {
    pub fn new(position: Point3<f32>, intensity: f32) -> Light {
        Light {
            position,
            intensity,
        }
    }
}

impl From<&UILight> for Light {
    fn from(value: &UILight) -> Self {
        Light::new(point![value.pos_x, value.pos_y, value.pos_z], value.intensity)
    }
}

#[derive(Clone, Copy)]
pub (crate) struct Camera {
    pub position: Point3<f32>,
    pub direction: Vector3<f32>,
    pub up: Vector3<f32>,
    pub fov_y_deg: f32,
}

impl Camera {
    pub fn new(position: Point3<f32>, direction: Vector3<f32>, up: Vector3<f32>, fov_y_deg: f32) -> Camera {
        Camera {
            position, 
            direction, 
            up,
            fov_y_deg,
        }
    }
}

impl From<&UICamera> for Camera {
    fn from(ui_camera: &UICamera) -> Self {
        Camera::new(
            point![
                    ui_camera.pos_x, 
                    ui_camera.pos_y, 
                    ui_camera.pos_z
                ],
            vector![
                    ui_camera.dir_x, 
                    ui_camera.dir_y, 
                    ui_camera.dir_z
                ],
            vector![
                ui_camera.up_x,
                ui_camera.up_y,
                ui_camera.up_z,
            ],
            ui_camera.fov_deg_y)
    }
}

/// The ray generation shader. 
pub fn ray_generation_shader(pos: PixelPos, dim: Dimensions, uniforms: &RaytracingUniforms) -> (f32, f32, f32) {
    let x = pos.x as f32;
    let y = pos.y as f32;
    let width = dim.width as f32;
    let height = dim.height as f32;
    let aspect_ratio = width / height;
    let fov_half_rad = (uniforms.camera.fov_y_deg / 2.0) / 180.0 * PI;
    let focal_distance = 1.0 / fov_half_rad.tan();
    
    //let pixel_offset = hammersley(frame_number, dim.width * dim.height);  //TODO implement differing positions from multiple frames
    
    let y = -((y / height) * 2.0 - 1.0);
    let x = ((x / width) * 2.0 - 1.0) * aspect_ratio;
    
    let up = uniforms.camera.up.normalize();
    let forward = uniforms.camera.direction.normalize();
    let right = forward.cross(&up).normalize(); //forward x up  
    let true_up = right.cross(&forward);
    let dir = forward * focal_distance - right * x + true_up * y;   //no idea why - but it works correct this way
    let dir = dir.normalize();

    let mut ray = Ray::new(uniforms.camera.position, dir, NEW_RAY_MAX_BOUNCES, pos);
    submit_ray(&mut ray, uniforms);

    (ray.intensity, ray.intensity, ray.intensity)
    //random_pcg3d(pos.x, pos.y, uniforms.frame_id)
}

fn intersection_shader(ray: &Ray, aabb: &Aabb) -> Option<f32> {
    match aabb.aabb_type {
        AABBType::Sphere => {
            let sphere_pos = (aabb.min + aabb.max.coords) * 0.5;
            let radius = aabb.max.x - sphere_pos.x;
            match ray_sphere_intersection(ray, &sphere_pos, radius) {
                SphereIntersection::NoIntersection => None,
                SphereIntersection::OneIntersection(t) => {
                    if t >= 0.0 {
                        Some(t)
                    } else {
                        None
                    }
                },
                SphereIntersection::TwoIntersections(t_1, t_2) => {
                    let min = t_1.min(t_2);
                    let max = t_1.max(t_2);
                    if min >= 0.0{
                        Some(min)
                    } else if max >= 0.0 {
                        Some(max)
                    } else {
                        None
                    }
                },
            }
        }
        AABBType::PlainBox => {
            let (t1, t2) = ray_aabb_intersection(ray, &aabb.min, &aabb.max).unwrap();
            //at least one value is guaranteed to be positive
            let min = t1.min(t2);
            if min >= 0.0 {
                Some(min)
            } else {
                Some(t1.max(t2))
            }
        }
    }
}

fn hit_shader(ray: &mut Ray, aabb: &Aabb, ray_intersection_length: f32, uniforms: &RaytracingUniforms) {
    ray.hit = true;
    ray.hit_distance = ray_intersection_length;
    
    let intersection_point = ray.origin + ray.direction * ray_intersection_length;
    let normal= match aabb.aabb_type {
        AABBType::PlainBox => {
            plain_box_normal_calculation(aabb, intersection_point)
        }
        AABBType::Sphere => {
            let sphere_pos = (aabb.min + aabb.max.coords) * 0.5;
            //let radius = aabb.max.x - sphere_pos.x;
            (intersection_point - sphere_pos).normalize()
        }
    };

    let new_shot_rays_pos = intersection_point + normal * 0.00001;
    
    let mut received_intensity = 0f32;
    for light in uniforms.lights.iter() {
        let direction = light.position - new_shot_rays_pos;
        let mut shadow_ray = Ray::new_shadow_ray(new_shot_rays_pos, direction);
        submit_ray(&mut shadow_ray, uniforms);
        if !shadow_ray.hit {
            let distance_adjusted = light.intensity / direction.magnitude_squared();
            let normal_adjusted = shadow_ray.direction.normalize().dot(&normal)
                .clamp(0.0, f32::INFINITY) * distance_adjusted;
            received_intensity += normal_adjusted;
        }
    }
    
    if ray.max_bounces > 1 {
        let (random_x, random_y, _) = random_pcg3d(ray.original_pixel_pos.x, 
                                                   ray.original_pixel_pos.y, uniforms.frame_id);
        let new_direction = random_bounce_from_normal(&normal, random_x, random_y);
        let mut new_ray = Ray::new(intersection_point, new_direction, 
                               ray.max_bounces - 1, ray.original_pixel_pos);
        submit_ray(&mut new_ray, uniforms);
        
        let distance_adjustment = 1.0 / (new_ray.hit_distance * new_ray.hit_distance);
        received_intensity += new_ray.intensity * new_direction.dot(&normal) //* distance_adjustment;   //TODO I think this is necessary but IDK
    }
    
    ray.intensity = received_intensity * (-ray.direction).dot(&normal);
}

/// The miss shader. It is called on a submitted ray if this ray does ultimately not hit anything. 
/// <br/>
/// Here it does nothing but set the intensity/color to 0 (black) and set the hit flag to false. 
fn miss_shader(ray: &mut Ray, _uniforms: &RaytracingUniforms) {
    ray.intensity = 0.0;
    ray.hit = false;
}

/// The heart of the raytracing engine, here the rays are actually shot and tracked through the 
/// scene. After all collisions have been determined, the appropriate shaders are called, which
/// mutate the ray and after this function returns, the result can be read from the submitted ray. 
fn submit_ray(ray: &mut Ray, uniforms: &RaytracingUniforms) {
    let mut intersections: Vec<(&Aabb, f32)> = Vec::new();
    
    for aabb in uniforms.aabbs.iter() {
        if let Some((_t_min, _t_max)) = ray_aabb_intersection(ray, &aabb.min, &aabb.max) {
            if let Some(t) = intersection_shader(ray, aabb) {
                intersections.push((aabb, t));
            }
        }
    }
    
    intersections.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    
    if let Some((aabb, t)) = intersections.first() {
        if !ray.skip_hit_shader {
            hit_shader(ray, aabb, *t, uniforms);
        } else {
            ray.hit = true;
        }

    } else {
        miss_shader(ray, uniforms);
    }
}

/// An enum to differentiate between the possible cases of a ray-sphere-intersection. The ray can
/// miss (NoIntersection), it can graze the sphere (OneIntersection) or go through 
/// (TwoIntersections). 
enum SphereIntersection {
    TwoIntersections(f32, f32),
    OneIntersection(f32),
    NoIntersection,
}

/// Calculates the intersection between a ray and a sphere. The intersection points are returned as
/// scalars for the direction of the ray. 
fn ray_sphere_intersection(ray: &Ray, sphere_pos: &Point3<f32>, sphere_rad: f32) -> SphereIntersection {
    let oc = ray.origin - sphere_pos;
    let a = ray.direction.dot(&ray.direction);
    let b = 2.0 * oc.dot(&ray.direction);
    let c = oc.dot(&oc) - sphere_rad * sphere_rad;
    
    let discriminant = b * b - 4.0 * a * c;
    
    if discriminant < 0.0 {
        SphereIntersection::NoIntersection
    } else if discriminant == 0.0 {
        let t = (-b - discriminant.sqrt()) / (2.0 * a);
        SphereIntersection::OneIntersection(t)
    } else {
        let discriminant_sqrt = discriminant.sqrt();
        let t1 = (-b - discriminant_sqrt) / (2.0 * a);
        let t2 = (-b + discriminant_sqrt) / (2.0 * a);
        SphereIntersection::TwoIntersections(t1, t2)
    }
}

fn ray_aabb_intersection(ray: &Ray, point_min: &Point3<f32>, point_max: &Point3<f32>) -> Option<(f32, f32)> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    
    for i in 0..3 {
        let inverse_direction = 1.0 / ray.direction[i];
        let t1 = (point_min[i] - ray.origin[i]) * inverse_direction;
        let t2 = (point_max[i] - ray.origin[i]) * inverse_direction;

        let (t_near, t_far) = if inverse_direction < 0.0 { (t2, t1) } else { (t1, t2) };
        
        t_min = t_min.max(t_near);
        t_max = t_max.min(t_far);
        
        if t_max <= t_min {
            return None;
        }
    }
    
    if t_max < 0.0 {
        return None;
    }
    
    Some((t_min, t_max)) 
}

fn plain_box_normal_calculation(aabb: &Aabb, intersection_point: OPoint<f32, Const<3>>) -> OMatrix<f32, Const<3>, Const<1>> {
    let x = if (intersection_point.x - aabb.min.x).abs() < F32_DELTA {
        -1.0
    } else if (intersection_point.x - aabb.max.x).abs() < F32_DELTA {
        1.0
    } else {
        0.0
    };
    let y = if (intersection_point.y - aabb.min.y).abs() < F32_DELTA {
        -1.0
    } else if (intersection_point.y - aabb.max.y).abs() < F32_DELTA {
        1.0
    } else {
        0.0
    };
    let z = if (intersection_point.z - aabb.min.z).abs() < F32_DELTA {
        -1.0
    } else if (intersection_point.z - aabb.max.z).abs() < F32_DELTA {
        1.0
    } else {
        0.0
    };
    vector![x, y, z].normalize()
}

// from http://holger.dammertz.org/stuff/notes_HammersleyOnHemisphere.html
// Hacker's Delight, Henry S. Warren, 2001
fn radical_inverse(mut bits: u32) -> f32 {
    bits = (bits << 16) | (bits >> 16);
    bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
    bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
    bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
    bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);
    (bits as f32) * 2.3283064365386963e-10  // / 0x100000000
}

fn hammersley(n: u32, N: u32) -> (f32, f32) {
    (
        (n as f32 + 0.5) / N as f32,
        radical_inverse(n + 1),
    )
}

/// Calculates three quasi random floats from unsigned integers. The integers can usually be: <br>
/// x = pixel position x, <br>
/// y = pixel position y, <br>
/// z = frame number <br>
/// The resulting three floats will probably be in range \[0; 1] <br>
/// <br>
/// Hash Functions for GPU Rendering, Jarzynski et al. <br>
/// http://www.jcgt.org/published/0009/03/02/
fn random_pcg3d(mut x: u32, mut y: u32, mut z: u32) -> (f32, f32, f32) {    //TODO This function may be improved using simd. Do some testing.
    x = x * 1664525 + 1013904223;
    y = y * 1664525 + 1013904223;
    z = z * 1664525 + 1013904223;
    x += y * z;
    y += z * x;
    z += x * y;
    x ^= x >> 16;
    y ^= y >> 16;
    z ^= z >> 16;
    x += y * z;
    y += z * x;
    z += x * y;

    let reciprocal = 1.0 / 0xffffffffu32 as f32;
    (
        x as f32 * reciprocal,
        y as f32 * reciprocal,
        z as f32 * reciprocal,
    )
}

fn random_bounce_from_normal(normal: &Vector3<f32>, random_x: f32, random_y: f32) -> Vector3<f32> {
    //determining random angles
    let azimuthal_angle = random_x * TAU;   //phi
    let polar_angle = random_y.sqrt().asin();   //theta
    
    //generate a random direction in its local space
    let (local_x, local_y, local_z) = (
        polar_angle.sin() * azimuthal_angle.cos(),
        polar_angle.sin() * azimuthal_angle.sin(),
        polar_angle.cos(),
    );
    let local_vec = Vector3::new(local_x, local_y, local_z);
    
    //align the local direction with the normal
    get_normal_space(normal) * local_vec
}

/// Returns a 3x3 Matrix which, when multiplied by, transforms a Vector3 into the space of the 
/// normal. A vector which simply points upwards will, after multiplication, point in the direction
/// of the normal //TODO check if this is necessarily true
fn get_normal_space(normal: &Vector3<f32>) -> Matrix3<f32> {
    let vec_basis = vector![1.0, 0.0, 0.0];
    let dd = vec_basis.dot(normal);
    let mut vec_tangent = vector![0.0, 0.0, 1.0];

    if 1.0 - dd.abs() > F32_DELTA {
        vec_tangent = vec_basis.cross(normal).normalize();
    }
    let bi_tangent = normal.cross(&vec_tangent);

    Matrix3::from_columns(&[vec_tangent, *normal, bi_tangent])
}