#![allow(unused)]

use std::fs;
use std::ops::{Add, Sub, Mul, Div, Neg};
use std::path::Path;
use std::process::Command;

use image::{ImageBuffer, Rgb};
use noise::{Fbm, NoiseFn, Perlin};
use once_cell::sync::Lazy;
use rayon::prelude::*;

// ---------------------------------------------------------------------------
// Render configuration. Drop WIDTH/HEIGHT for quick iteration, crank for the
// final render. TOTAL_FRAMES = FPS * DURATION_SECS.
// ---------------------------------------------------------------------------
const WIDTH: usize = 1280;
const HEIGHT: usize = 720;
const FPS: u32 = 30;
const DURATION_SECS: u32 = 10;
const TOTAL_FRAMES: u32 = FPS * DURATION_SECS;

// ---------------------------------------------------------------------------
// Vec3 + operator overloads. cross() is required for the camera basis.
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec3 { x: f64, y: f64, z: f64 }

impl Vec3 {
    fn new(x: f64, y: f64, z: f64) -> Self { Self { x, y, z } }
    fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
    fn length(self) -> f64 { self.dot(self).sqrt() }
    fn unit(self) -> Vec3 { self / self.length() }
}

impl Add for Vec3 { type Output = Vec3; fn add(self, r: Vec3) -> Vec3 { Vec3::new(self.x+r.x, self.y+r.y, self.z+r.z) } }
impl Sub for Vec3 { type Output = Vec3; fn sub(self, r: Vec3) -> Vec3 { Vec3::new(self.x-r.x, self.y-r.y, self.z-r.z) } }
impl Mul<f64> for Vec3 { type Output = Vec3; fn mul(self, t: f64) -> Vec3 { Vec3::new(self.x*t, self.y*t, self.z*t) } }
impl Mul<Vec3> for f64 { type Output = Vec3; fn mul(self, v: Vec3) -> Vec3 { v * self } }
impl Div<f64> for Vec3 { type Output = Vec3; fn div(self, t: f64) -> Vec3 { self * (1.0 / t) } }
impl Neg for Vec3 { type Output = Vec3; fn neg(self) -> Vec3 { Vec3::new(-self.x, -self.y, -self.z) } }

// ---------------------------------------------------------------------------
// Ray / Sphere / Material
// ---------------------------------------------------------------------------
#[derive(Clone, Debug)]
struct Ray { origin: Option<Vec3>, direction: Option<Vec3> }

#[derive(Clone, Copy, Debug, PartialEq)]
enum Material { Sun, Earth, Jupiter, Solid }

#[derive(Clone, Debug)]
struct Sphere {
    centre: Option<Vec3>,
    rad: Option<f64>,
    red: Option<usize>,
    green: Option<usize>,
    blue: Option<usize>,
    material: Material,
}

fn hits(sphere: &Sphere, ray: &Ray) -> f64 {
    let oc = ray.origin.unwrap() - sphere.centre.unwrap();
    let a = ray.direction.unwrap().dot(ray.direction.unwrap());
    let b = 2.0 * ray.direction.unwrap().dot(oc);
    let c = oc.dot(oc) - sphere.rad.unwrap() * sphere.rad.unwrap();
    let disc = b * b - 4.0 * a * c;
    if disc > 0.0 {
        let root1 = (-disc.sqrt() - b) / (2.0 * a);
        let root2 = ( disc.sqrt() - b) / (2.0 * a);
        if root1 > 0.001 { return root1; }
        if root2 > 0.001 { return root2; }
        return -1.0;
    }
    -1.0
}

// ---------------------------------------------------------------------------
// Procedural textures. Built once, sampled per-pixel across all threads.
// ---------------------------------------------------------------------------
static EARTH_NOISE: Lazy<Fbm<Perlin>> = Lazy::new(|| {
    let mut fbm = Fbm::<Perlin>::new(42);
    fbm.octaves = 5;
    fbm.frequency = 1.8;
    fbm.persistence = 0.55;
    fbm
});

static JUPITER_NOISE: Lazy<Fbm<Perlin>> = Lazy::new(|| {
    let mut fbm = Fbm::<Perlin>::new(7);
    fbm.octaves = 4;
    fbm.frequency = 2.5;
    fbm.persistence = 0.5;
    fbm
});

fn earth_color(local: Vec3) -> (f64, f64, f64) {
    let n = EARTH_NOISE.get([local.x, local.y, local.z]).clamp(-1.0, 1.0);
    if n < -0.05      { ( 20.0,  60.0, 140.0) }   // deep ocean
    else if n < 0.05  { ( 40.0, 110.0, 180.0) }   // shallow water
    else if n < 0.25  { ( 60.0, 130.0,  60.0) }   // forest
    else if n < 0.5   { (110.0, 130.0,  70.0) }   // highlands
    else              { (220.0, 220.0, 210.0) }   // snow
}

fn jupiter_color(local: Vec3) -> (f64, f64, f64) {
    let turbulence = JUPITER_NOISE.get([local.x * 0.8, local.y * 0.8, local.z * 0.8]) * 0.15;
    let lat = local.y + turbulence;
    let band = (lat * 8.0).sin();
    let t = (band + 1.0) * 0.5;
    let dark = (180.0, 130.0, 70.0);
    let light = (240.0, 200.0, 150.0);
    (
        dark.0 * (1.0 - t) + light.0 * t,
        dark.1 * (1.0 - t) + light.1 * t,
        dark.2 * (1.0 - t) + light.2 * t,
    )
}

// ---------------------------------------------------------------------------
// Scene: fixed sun, planets on circular orbits in xz.
// ---------------------------------------------------------------------------
fn sun_center() -> Vec3 { Vec3::new(0.0, 0.0, -8.0) }

fn build_scene(t: f64) -> Vec<Sphere> {
    // (orbit_radius, orbit_speed, y_offset, body_radius, r, g, b, material)
    let planets: [(f64, f64, f64, f64, usize, usize, usize, Material); 8] = [
        (1.6, 1.60,  0.0,  0.18, 169, 169, 169, Material::Solid),    // Mercury
        (2.2, 1.20, -0.3,  0.30, 255, 210, 120, Material::Solid),    // Venus
        (3.0, 1.00,  0.2,  0.32,  50, 120, 200, Material::Earth),    // Earth
        (3.8, 0.80, -0.4,  0.20, 188,  74,  40, Material::Solid),    // Mars
        (5.2, 0.45,  0.1,  0.75, 220, 165, 100, Material::Jupiter),  // Jupiter
        (6.6, 0.32, -0.2,  0.62, 210, 185, 130, Material::Solid),    // Saturn
        (7.8, 0.22,  0.3,  0.45, 130, 210, 220, Material::Solid),    // Uranus
        (9.0, 0.16, -0.1,  0.40,  50,  80, 200, Material::Solid),    // Neptune
    ];

    let sun = sun_center();
    let mut spheres = vec![Sphere {
        centre: Some(sun),
        rad: Some(1.5),
        red: Some(255), green: Some(220), blue: Some(50),
        material: Material::Sun,
    }];
    for (r_orbit, speed, y_off, r_body, cr, cg, cb, mat) in planets {
        let angle = t * speed;
        let cx = sun.x + r_orbit * angle.cos();
        let cz = sun.z + r_orbit * angle.sin();
        spheres.push(Sphere {
            centre: Some(Vec3::new(cx, sun.y + y_off, cz)),
            rad: Some(r_body),
            red: Some(cr), green: Some(cg), blue: Some(cb),
            material: mat,
        });
    }
    spheres
}

// ---------------------------------------------------------------------------
// Shader. Returns RGB as bytes ready for the PNG buffer.
// ---------------------------------------------------------------------------
fn ray_color(ray: &Ray, scene: &[Sphere]) -> [u8; 3] {
    let mut closest_t = f64::MAX;
    let mut hit_anything = false;
    let mut closest_sphere: Option<&Sphere> = None;
    for object in scene {
        let t = hits(object, ray);
        if t != -1.0 && t < closest_t {
            closest_t = t;
            hit_anything = true;
            closest_sphere = Some(object);
        }
    }
    let sun_pos = scene[0].centre.unwrap();

    if !hit_anything {
        let sun_dir = (sun_pos - ray.origin.unwrap()).unit();
        let alignment = ray.direction.unwrap().unit().dot(sun_dir).max(0.0);
        let glow_r = alignment.powf(9.0)  * 120.0;
        let glow_g = alignment.powf(14.0) *  70.0;
        let glow_b = alignment.powf(23.0) *  30.0;
        let base = 5.0;
        return [
            (base + glow_r).min(255.0) as u8,
            (base + glow_g).min(255.0) as u8,
            (base + glow_b).min(255.0) as u8,
        ];
    }

    let sphere = closest_sphere.unwrap();
    let hitpoint = ray.origin.unwrap() + closest_t * ray.direction.unwrap();
    let normal = (hitpoint - sphere.centre.unwrap()).unit();

    if sphere.material == Material::Sun {
        let view_dir = (-ray.direction.unwrap()).unit();
        let limb = normal.dot(view_dir).max(0.0);
        let g = (15.0 + 235.0 * limb.powf(0.4)).min(255.0) as u8;
        let b = (180.0 * limb.powf(1.6)).min(255.0) as u8;
        return [255, g, b];
    }

    // For a sphere, the unit normal IS the local-space coordinate on a unit
    // sphere. Pass it straight to the texture functions — scale-invariant.
    let (base_r, base_g, base_b) = match sphere.material {
        Material::Earth   => earth_color(normal),
        Material::Jupiter => jupiter_color(normal),
        _ => (
            sphere.red.unwrap()   as f64,
            sphere.green.unwrap() as f64,
            sphere.blue.unwrap()  as f64,
        ),
    };

    let to_light = sun_pos - hitpoint;
    let light_dir = to_light / to_light.length();
    let diffuse = normal.dot(light_dir).max(0.0);
    let view_dir = (-ray.direction.unwrap()).unit();
    let reflect = -light_dir - normal * (2.0 * (-light_dir).dot(normal));
    let spec = view_dir.dot(reflect).max(0.0).powf(48.0) * 0.85;
    let bright = 0.05 + diffuse;

    [
        ((base_r * bright) + spec * 255.0).min(255.0) as u8,
        ((base_g * bright) + spec * 255.0).min(255.0) as u8,
        ((base_b * bright) + spec * 255.0).min(255.0) as u8,
    ]
}

// ---------------------------------------------------------------------------
// Camera orbits the sun on a circle, slightly above the orbital plane.
// ---------------------------------------------------------------------------
fn camera_at(t_norm: f64) -> Vec3 {
    let target = sun_center();
    let orbit_radius = 15.0;
    let orbit_height = 2.0;
    let angle = t_norm * std::f64::consts::TAU;
    target + Vec3::new(
        orbit_radius * angle.cos(),
        orbit_height,
        orbit_radius * angle.sin(),
    )
}

// ---------------------------------------------------------------------------
// One frame: build scene, set up camera basis, parallel-render rows, save PNG
// directly via the image crate. Skips if PNG already exists (resume on crash).
// ---------------------------------------------------------------------------
fn render_frame(frame: u32) {
    let png_path = format!("frames/frame_{:04}.png", frame);
    if Path::new(&png_path).exists() {
        println!("frame {} already exists, skipping", frame);
        return;
    }

    let t = frame as f64 / FPS as f64;
    let t_norm = frame as f64 / TOTAL_FRAMES as f64;
    let scene = build_scene(t);
    let origin = camera_at(t_norm);
    let target = sun_center();

    // Orthonormal camera basis pointing at the sun.
    let world_up = Vec3::new(0.0, 1.0, 0.0);
    let forward = (target - origin).unit();
    let right = forward.cross(world_up).unit();
    let up = right.cross(forward);

    let aspect_ratio = WIDTH as f64 / HEIGHT as f64;
    let viewport_height = 2.0;
    let viewport_width = aspect_ratio * viewport_height;
    let focal_length = 1.0;
    let horizontal = right * viewport_width;
    let vertical = up * viewport_height;
    let lower_left_corner =
        origin + forward * focal_length - horizontal / 2.0 - vertical / 2.0;

    // Render rows in parallel. Top-to-bottom order (j descending) so the PNG
    // ends up oriented correctly when written row-major.
    let rows: Vec<Vec<[u8; 3]>> = (0..HEIGHT).into_par_iter().rev().map(|j| {
        let mut row = Vec::with_capacity(WIDTH);
        for i in 0..WIDTH {
            let u = i as f64 / (WIDTH as f64 - 1.0);
            let v = j as f64 / (HEIGHT as f64 - 1.0);
            let direction = lower_left_corner + u * horizontal + v * vertical - origin;
            let ray = Ray { origin: Some(origin), direction: Some(direction) };
            row.push(ray_color(&ray, &scene));
        }
        row
    }).collect();

    // Flatten into a single Vec<u8> in row-major order for image::save_buffer.
    let mut buf: Vec<u8> = Vec::with_capacity(WIDTH * HEIGHT * 3);
    for row in rows {
        for px in row {
            buf.extend_from_slice(&px);
        }
    }

    image::save_buffer(
        &png_path,
        &buf,
        WIDTH as u32,
        HEIGHT as u32,
        image::ColorType::Rgb8,
    ).expect("PNG save failed");

    println!("frame {}/{} done", frame + 1, TOTAL_FRAMES);
}

fn main() {
    fs::create_dir_all("frames").unwrap();
    for frame in 0..TOTAL_FRAMES {
        render_frame(frame);
    }
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-framerate", &FPS.to_string(),
            "-i", "frames/frame_%04d.png",
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            "-crf", "18",
            "out.mp4",
        ])
        .status()
        .expect("ffmpeg not found on PATH — install ffmpeg");
    if status.success() {
        println!("wrote out.mp4");
    } else {
        eprintln!("ffmpeg failed");
    }
}
