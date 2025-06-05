use std::f32::consts::PI;
use std::sync::Arc;
use nalgebra::{point, vector, Const, Matrix3, OMatrix, OPoint, Point3, Vector3};
use crate::{UICamera, UILight, UIObject, UIObjectType};
use crate::spectrum::Spectrum;

pub(crate) const F32_DELTA: f32 = 0.00001;
const NEW_RAY_MAX_BOUNCES: u32 = 30;
const NEW_RAY_POSITION_OFFSET_DISTANCE: f32 = 0.00001;
const HAMMERSLEY_OFFSET_SCALE: f32 = 0.001;

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
    pub(crate) intended_frames_amount: u32,
    pub(crate) example_spectrum: Spectrum,
}

/// The struct representing the ray that is shot through the scene. It contains information about
/// the origin and direction as well as returned information such as color (intensity). 
struct Ray {
    origin: Point3<f32>,
    direction: Vector3<f32>,
    hit: bool,
    spectrum: Spectrum,
    skip_hit_shader: bool,
    max_bounces: u32,
    original_pixel_pos: PixelPos,
    hit_distance: f32,
    max_hit_distance: f32,
}
impl Ray {
    /// Creates a new standard Ray with default values for the values which will be written to in 
    /// the shaders. 
    fn new(origin: Point3<f32>, direction: Vector3<f32>, max_bounces: u32,
           original_pixel_pos: PixelPos, example_spectrum: &Spectrum) -> Ray {
        Ray {
            origin,
            direction: direction.normalize(),
            hit: false,
            spectrum: Spectrum::new_equal_size_empty_spectrum(example_spectrum),
            skip_hit_shader: false,
            max_bounces,
            original_pixel_pos,
            hit_distance: 0.0,
            max_hit_distance: f32::INFINITY,
        }
    }
    
    /// Creates a new shadow ray. Shadow rays are rays which terminate upon hitting anything and 
    /// can thus be used to determine if an unobstructed line to another point exists. The 
    /// closest-hit shader will not be executed for this ray. The field hit will be set to true if 
    /// anything is hit. 
    fn new_shadow_ray(origin: Point3<f32>, direction: Vector3<f32>, max_hit_distance: f32, 
                      example_spectrum: &Spectrum) -> Ray 
    {
        Ray {
            origin, 
            direction,
            hit: false,
            spectrum: Spectrum::new_equal_size_empty_spectrum(example_spectrum),    //TODO maybe refactor this out
            skip_hit_shader: true,
            max_bounces: 2, //technically unnecessary
            original_pixel_pos: PixelPos {x:0, y:0},    //dummy value
            hit_distance: 0.0,
            max_hit_distance,
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
    spectrum: Spectrum,
    metallicness: bool,  //TODO remake as f32, but now only totally diffuse or totally metallic
}   //TODO refactor material info into single struct "material"
impl Aabb {
    pub fn new_sphere(center: &Point3<f32>, radius: f32, spectrum: Spectrum, metallicness: bool) -> Aabb {
        Aabb {
            min: point![center.x - radius, center.y - radius, center.z - radius],
            max: point![center.x + radius, center.y + radius, center.z + radius],
            aabb_type: AABBType::Sphere,
            spectrum,
            metallicness,
        }
    }
    
    pub fn new_box(center: &Point3<f32>, x_length: f32, y_length: f32, z_length: f32, spectrum: Spectrum, metallicness: bool) -> Aabb {
        let x_half = x_length / 2.0;
        let y_half = y_length / 2.0;
        let z_half = z_length / 2.0;
        Aabb {
            min: point![center.x - x_half, center.y - y_half, center.z - z_half],
            max: point![center.x + x_half, center.y + y_half, center.z + z_half],
            aabb_type: AABBType::PlainBox,
            spectrum,
            metallicness,
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
                Aabb::new_box(&pos, x_length, y_length, z_length, value.spectrum.borrow().spectrum.clone(), value.metallicness)
            }
            UIObjectType::Sphere(radius) => {
                Aabb::new_sphere(&pos, radius, value.spectrum.borrow().spectrum.clone(), value.metallicness)
            }
        }
    }
}

pub (crate) struct Light {
    position: Point3<f32>,
    spectrum: Spectrum,
}
impl Light {
    pub fn new(position: Point3<f32>, spectrum: Spectrum) -> Light {
        Light {
            position,
            spectrum,
        }
    }
}

impl From<&UILight> for Light {
    fn from(value: &UILight) -> Self {
        Light::new(point![value.pos_x, value.pos_y, value.pos_z], 
                   value.spectrum.borrow().spectrum.clone())
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
    
    let (pixel_offset_x, pixel_offset_y) = 
        hammersley(uniforms.frame_id, uniforms.intended_frames_amount);
    
    let y = -((y / height) * 2.0 - 1.0) + pixel_offset_x * HAMMERSLEY_OFFSET_SCALE; //TODO this can be calculated smarter than a constant
    let x = ((x / width) * 2.0 - 1.0) * aspect_ratio + pixel_offset_y * HAMMERSLEY_OFFSET_SCALE;
    
    let up = uniforms.camera.up.normalize();
    let forward = uniforms.camera.direction.normalize();
    let right = forward.cross(&up).normalize(); //forward x up  
    let true_up = right.cross(&forward);
    let dir = forward * focal_distance - right * x + true_up * y;   //no idea why the - but it works correct this way
    let dir = dir.normalize();

    let mut ray = Ray::new(uniforms.camera.position, dir, NEW_RAY_MAX_BOUNCES, pos, &uniforms.example_spectrum);
    submit_ray(&mut ray, uniforms);

    ray.spectrum.to_rgb_early()
    //random_pcg3d(pos.x, pos.y, uniforms.frame_id)
    //TODO dead center in the middle sphere is a big fat aliasing circle
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
    
    //determining position and normal of the hit
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

    //a new ray is shot slightly above the hit position because of floating point imprecision in 
    //order not to intersect at the hit position
    let new_shot_rays_pos = intersection_point + normal * NEW_RAY_POSITION_OFFSET_DISTANCE;
    
    
    //calculating how much light hits this point
    let mut received_spectrum = Spectrum::new_equal_size_empty_spectrum(&ray.spectrum);
    
    if aabb.metallicness {  //TODO metallic rays cannot yet detect light sources
        if ray.max_bounces > 1 {
            let direction = reflect_vec(&ray.direction, &normal);
            let mut new_ray = Ray::new(new_shot_rays_pos, direction, 
                                       ray.max_bounces - 1, ray.original_pixel_pos, &ray.spectrum);
            submit_ray(&mut new_ray, uniforms);

            received_spectrum += &new_ray.spectrum;
        }   //else just simply black 
    } else {
        //direct light contributions via light sources
        //important: ONLY HERE is the light intensity divided by distance squared, reflected rays
        // have already paid the square tax. 
        for light in uniforms.lights.iter() {
            let direction = light.position - new_shot_rays_pos;
            let distance = direction.magnitude();
            let direction_norm = direction.normalize();
            let mut shadow_ray = Ray::new_shadow_ray(new_shot_rays_pos, direction_norm, distance, &ray.spectrum);
            submit_ray(&mut shadow_ray, uniforms);
            
            if !shadow_ray.hit {
                //adjust strength for distance from light source
                let mut adjusted = &light.spectrum / direction.magnitude_squared();
                
                //adjust for incoming ray angle
                adjusted *= shadow_ray.direction.normalize().dot(&normal)
                    //.clamp(0.0, f32::INFINITY);
                    .max(0.0);
                
                //adjust for outgoing ray angle
                adjusted *= (-ray.direction).dot(&normal)
                    //.clamp(0.0, f32::INFINITY);
                    .max(0.0);
                
                received_spectrum += &adjusted;
            }
        }

        //indirect light contribution (diffuse - random - light ray bounces)
        if ray.max_bounces > 1 {
            let (random_x, random_y, _) = random_pcg3d(ray.original_pixel_pos.x,    //TODO do in front of if and use third random for metallicness
                                                       ray.original_pixel_pos.y, uniforms.frame_id);
            let theta = random_x.sqrt().asin(); //importance sampling of a sphere, therefore no direction correction necessary later
            let phi = 2.0 * PI * random_y;
            let local_direction = vector![theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos()];
            let new_direction = get_normal_space2(&normal) * local_direction;
            //let new_direction = random_bounce_from_normal(&normal, random_x, random_y);
            let mut new_ray = Ray::new(intersection_point, new_direction,
                                   ray.max_bounces - 1, ray.original_pixel_pos, &ray.spectrum);
            submit_ray(&mut new_ray, uniforms);

            new_ray.spectrum.max0();
            //no direction correction here
            received_spectrum += &new_ray.spectrum; 
        }
    }
    
    ray.spectrum = &aabb.spectrum * &received_spectrum;
}

/// https://www.gsn-lib.org/apps/raytracing/index.php?name=example_emissivesphere
fn get_normal_space2(normal: &Vector3<f32>) -> Matrix3<f32> {
    let some_vec = Vector3::<f32>::new(1.0, 0.0, 0.0);
    let dd = some_vec.dot(normal);
    let mut tangent = Vector3::<f32>::new(0.0, 1.0, 0.0);
    if 1.0 - dd.abs() > F32_DELTA {
        tangent = some_vec.cross(normal).normalize()
    }
    let bi_tangent = normal.cross(&tangent);
    Matrix3::from_columns(&[tangent, bi_tangent, *normal])
}

/// The miss shader. It is called on a submitted ray if this ray does ultimately not hit anything. 
/// <br/>
/// Here it does nothing but set the intensity/color to 0 (black) and set the hit flag to false. 
fn miss_shader(ray: &mut Ray, _uniforms: &RaytracingUniforms) {
    ray.spectrum = Spectrum::new_equal_size_empty_spectrum(&ray.spectrum);  //TODO make sky blue perhaps or give user choice
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
                if t > 0.0 {
                    intersections.push((aabb, t));
                }
            }
        }
    }
    
    intersections.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    
    if let Some((aabb, t)) = intersections.first() {
        if t <= &ray.max_hit_distance {
            if !ray.skip_hit_shader {
                hit_shader(ray, aabb, *t, uniforms);
            } else {
                ray.hit = true;
            }
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
//adapted to be used in rust
fn radical_inverse(mut bits: u32) -> f32 {
    bits = bits.rotate_right(16);
    bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
    bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
    bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
    bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);
    (bits as f32) * 2.328_306_4e-10  // / 0x100000000
}

fn hammersley(n: u32, capital_n: u32) -> (f32, f32) {
    (
        (n as f32 + 0.5) / capital_n as f32,
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

/// Reflects a vector incident about the given normal (which must be normalized for correct results).
/// The incident must point towards the normal, not away as one might think.
fn reflect_vec(incident: &Vector3<f32>, normal: &Vector3<f32>) -> Vector3<f32> {
    incident - 2.0 * normal.dot(incident) * normal
}