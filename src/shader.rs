use std::f32::consts::FRAC_PI_2;
use nalgebra::{point, Point3, Vector3};
use crate::hsb;

pub struct PixelPos {
    pub x: u32,
    pub y: u32,
}

pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

pub fn shader_gradient(pos: PixelPos, dim: Dimensions, _: ()) -> (f32, f32, f32) {
    let norm_x = pos.x as f32 / dim.width as f32;
    let norm_y = pos.y as f32 / dim.height as f32;

    (norm_x, norm_y, 0.0)
}

pub fn shader_spiral(pos: PixelPos, dim: Dimensions, _: ()) -> (f32, f32, f32) {
    let bounded_x = (pos.x as f32 / dim.width as f32) * 2.0 - 1.0;
    let bounded_y = (pos.y as f32 / dim.height as f32) * 2.0 - 1.0;

    let angle = (bounded_x/bounded_y).atan();
    let angle_norm = ((angle / FRAC_PI_2) + 1.0) / 2.0;
    
     hsb::hsv_to_rgb(angle_norm, 1.0, 1.0)
}

pub struct RaytracingUniforms {
    pub(crate) aabbs: Vec<Aabb>,
}

struct Ray {
    origin: Point3<f32>,
    direction: Vector3<f32>,
    hit: bool,
    intensity: f32,
}
impl Ray {
    fn new(origin: Point3<f32>, direction: Vector3<f32>, hit: bool) -> Ray {
        Ray {
            origin,
            direction,
            hit,
            intensity: 0.0,
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
    pub fn test_instance1() -> Aabb {
        Aabb {
            min: point![0.0, -0.5, -0.5],
            max: point![1.0, 0.5, 0.5],
            aabb_type: AABBType::Sphere,
        }
    }
    pub fn test_instance2() -> Aabb {
        Aabb {
            min: point![-1.0, -0.5, -0.5],
            max: point![0.0, 0.5, 0.5],
            aabb_type: AABBType::PlainBox,
        }
    }
}
enum AABBType {
    PlainBox,
    Sphere,
}

/// The ray generation shader. 
pub fn shader_raytracing(pos: PixelPos, dim: Dimensions, uniforms: &RaytracingUniforms) -> (f32, f32, f32) {
    let x = pos.x as f32;
    let y = pos.y as f32;
    let width = dim.width as f32;
    let height = dim.height as f32;
    let aspect_ratio = width / height;
    
    let y = -((y / height) * 2.0 - 1.0);
    let x = ((x / width) * 2.0 - 1.0) * aspect_ratio;
    
    let mut ray = Ray::new(Point3::new(x, y, 0.0), Vector3::new(0.0, 0.0, 1.0), false);
    submit_ray(&mut ray, uniforms);

    (ray.intensity, ray.intensity, ray.intensity)
}

fn submit_ray(ray: &mut Ray, uniforms: &RaytracingUniforms) {
    for aabb in uniforms.aabbs.iter() {
        if ray_aabb_intersection(ray, &aabb.min, &aabb.max) {
            //TODO maybe this is already hit shader territory
            match aabb.aabb_type {
                AABBType::PlainBox => {
                    ray.hit = true;
                    ray.intensity = 1.0;
                }
                AABBType::Sphere => {
                    let sphere_pos = (aabb.min + aabb.max.coords) * 0.5;
                    let radius = aabb.max.x - sphere_pos.x;
                    match ray_sphere_intersection(ray, &sphere_pos, radius) {
                        SphereIntersection::NoIntersection => (),
                        SphereIntersection::OneIntersection(t) => {
                            ray.hit = true;
                            let intersection_point = ray.origin + ray.direction * t;
                            let normal = (intersection_point - sphere_pos).normalize();
                            ray.intensity = ray.direction.dot(&normal).abs();
                        }
                        SphereIntersection::TwoIntersections(t1, t2) => {
                            ray.hit = true;
                            let t = t1.min(t2);
                            let intersection_point = ray.origin + ray.direction * t;
                            let normal = (intersection_point - sphere_pos).normalize();
                            ray.intensity = ray.direction.dot(&normal).abs();
                        }
                    }
                }
            }
        }
    }
    
    
    // match ray_sphere_intersection(ray, &sphere_pos, sphere_rad) {
    //     SphereIntersection::NoIntersection => ray.hit = false,
    //     SphereIntersection::OneIntersection(t) => {
    //         ray.hit = true;
    //         let intersection_point = ray.origin + ray.direction * t;
    //         let normal = (intersection_point - sphere_pos).normalize();
    //         ray.intensity = ray.direction.dot(&normal).abs();
    //     }
    //     SphereIntersection::TwoIntersections(t1, t2) => {
    //         ray.hit = true;
    //         let t = t1.min(t2);
    //         let intersection_point = ray.origin + ray.direction * t;
    //         let normal = (intersection_point - sphere_pos).normalize();
    //         ray.intensity = ray.direction.dot(&normal).abs();
    //     }
    // }
}

enum SphereIntersection {
    TwoIntersections(f32, f32),
    OneIntersection(f32),
    NoIntersection,
}

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

fn ray_aabb_intersection(ray: &Ray, point_min: &Point3<f32>, point_max: &Point3<f32>) -> bool {
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
            return false;
        }
    }
    
    if t_max < 0.0 {
        return false;
    }
    
    true    //t_min and t_max could potentially be returned here
}