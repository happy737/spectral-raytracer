use std::f32::consts::{PI};
use nalgebra::{point, vector, Const, OMatrix, OPoint, Point3, Vector3};

const F32_DELTA: f32 = 0.00001;

pub struct PixelPos {
    pub x: u32,
    pub y: u32,
}

pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

pub struct RaytracingUniforms {
    pub(crate) aabbs: Vec<Aabb>,
    pub(crate) lights: Vec<Light>,
    pub(crate) camera: Camera,
}

struct Ray {
    origin: Point3<f32>,
    direction: Vector3<f32>,
    hit: bool,
    intensity: f32,
    skip_hit_shader: bool,
}
impl Ray {
    fn new(origin: Point3<f32>, direction: Vector3<f32>, skip_hit_shader: bool) -> Ray {
        Ray {
            origin,
            direction: direction.normalize(),
            hit: false,
            intensity: 0.0,
            skip_hit_shader,
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

pub (crate) struct Camera {
    pub position: Point3<f32>,
    pub direction: Vector3<f32>,
    pub fov_y_deg: f32,
}

impl Camera {
    pub fn new(position: Point3<f32>, direction: Vector3<f32>, fov_y_deg: f32) -> Camera {
        Camera {
            position, 
            direction, 
            fov_y_deg,
        }
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
    
    let y = -((y / height) * 2.0 - 1.0);
    let x = ((x / width) * 2.0 - 1.0) * aspect_ratio;
    
    //TODO do something with the camera position and direction arguments
    let mut ray = Ray::new(Point3::new(x, y, 0.0), Vector3::new(x, y, focal_distance), false);
    submit_ray(&mut ray, uniforms);

    (ray.intensity, ray.intensity, ray.intensity)
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

    let mut received_intensity = 0f32;
    for light in &uniforms.lights {
        let shadow_ray_pos = intersection_point + normal * 0.00001;
        let direction = light.position - shadow_ray_pos;
        let mut shadow_ray = Ray::new(shadow_ray_pos, direction, true);
        submit_ray(&mut shadow_ray, uniforms);
        if !shadow_ray.hit {
            let distance_adjusted = light.intensity / direction.magnitude_squared();
            let normal_adjusted = shadow_ray.direction.normalize().dot(&normal)
                .clamp(0.0, f32::INFINITY) * distance_adjusted;
            received_intensity += normal_adjusted;
        }
    }
    
    ray.intensity = received_intensity * (-ray.direction).dot(&normal);
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

fn miss_shader(ray: &mut Ray, uniforms: &RaytracingUniforms) {
    ray.intensity = 0.0;
    ray.hit = false;
}

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
