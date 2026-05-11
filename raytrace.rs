#![allow(unused)]
use std::fs::File;
use std::io::Write;
use std::io::BufWriter;
use std::process::Command;
fn write_ppm(file: &mut std::io::BufWriter<File>, color_string: String) {
    writeln!(file, "{}", color_string).expect("filewrite fail");
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}
impl Vec3 {
    fn new(x:f64,y:f64,z:f64) -> Self {
        Self {x,y,z}
    }
    fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    fn unit(self) -> Vec3 {
        self/self.length()
    }
}
use std::ops::{Add, Sub, Mul, Div, Neg};
impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, rhs: Vec3) -> Vec3 { Vec3::new(self.x+rhs.x, self.y+rhs.y, self.z+rhs.z) }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, rhs: Vec3) -> Vec3 { Vec3::new(self.x-rhs.x, self.y-rhs.y, self.z-rhs.z) }
}
impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, t: f64) -> Vec3 { Vec3::new(self.x*t, self.y*t, self.z*t) }
}
impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 { v * self }
}
impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, t: f64) -> Vec3 { self * (1.0/t) }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 { Vec3::new(-self.x, -self.y, -self.z) }
}
#[derive(Clone, Debug)]
struct Ray{
    origin:Option<Vec3>,
    direction:Option<Vec3>,
}
#[derive(Clone, Debug)]
struct Sphere{
    centre:Option<Vec3>,
    rad:Option<f64>,
    red:Option<usize>,
    green:Option<usize>,
    blue:Option<usize>,
}
fn hits(sphere:&Sphere,ray:&Ray)->f64{
    let oc:Vec3=ray.origin.unwrap()-sphere.centre.unwrap();
    let a:f64=ray.direction.unwrap().dot(ray.direction.unwrap());
    let b:f64=2.0*ray.direction.unwrap().dot(oc);
    let c:f64=oc.dot(oc)-sphere.rad.unwrap()*sphere.rad.unwrap();
    let disc:f64=b*b-4.0*a*c;
    if disc>0.0{
        let root1=(-disc.sqrt()-b)/(2.0*a);
        let root2=(disc.sqrt()-b)/(2.0*a);
        if root1>0.001{return root1}
        if root2>0.001{return root2}
        else{return -1.0}
    }else{return -1.0}
}
fn ray_color(ray:&Ray)->String{
    //VIBECODED PART START
    let mut objects = vec![];

    let data: [(f64, f64, f64, f64, usize, usize, usize); 9] = [
    // Sun — center at the right viewport edge at z=-3.0, half off-screen
    ( 8.84,  0.0, -1.0,  4.95, 255, 220,  50),

    // Mercury — small, gray, inner orbit
    ( 2.34,  0.9, -3.3,  0.18, 169, 169, 169),

    // Venus — warm yellowish, slightly larger
    ( 1.52, -0.5, -3.0,  0.30, 255, 210, 120),

    // Earth — blue
    ( 1.28,  0.6, -4.5,  0.32,  50, 120, 200),

    // Mars — rusty red, small
    (-0.14, -0.7, -2.5,  0.20, 188,  74,  40),

    // Jupiter — large, orange-tan
    (-1.21,  0.3, -3.5,  0.75, 220, 165, 100),

    // Saturn — large, pale gold
    (-2.63, -0.6, -3.0,  0.62, 210, 185, 130),

    // Uranus — teal-blue
    (-3.22,  0.8, -4.0,  0.45, 130, 210, 220),

    // Neptune — deep blue, far left
    (-5.00, -0.3, -3.5,  0.40,  50,  80, 200),
];
//VIBECODED PART END

    for d in data {
        objects.push(Sphere {
            centre: Some(Vec3::new(d.0, d.1, d.2)),
            rad: Some(d.3),
            red: Some(d.4),
            green: Some(d.5),
            blue: Some(d.6),
        });
    }
    let mut closest_t = f64::MAX;
    let mut hit_anything = false;
    let mut closest_sphere: Option<&Sphere> = None;
    for object in &objects{
        let t=hits(&object,&ray);
        if t!=-1.0 && t<closest_t{
            closest_t = t;
            hit_anything = true;
            closest_sphere = Some(&object);
        }
    }
    let sun_pos = Vec3::new(8.84, 0.0, -1.0);

    if !hit_anything {
        let unit_dir_x=ray.direction.unwrap().unit().dot(Vec3::new(1.0, 0.0, 0.0));
        let a=(unit_dir_x+1.0) * 0.5;
        let brightness=a * 40.0;
        let sun_dir = sun_pos.unit();
        let alignment = ray.direction.unwrap().unit().dot(sun_dir).max(0.0);
        let glow_r = alignment.powf(9.0) * 90.0;
        let glow_g = alignment.powf(14.0) * 50.0;
        let glow_b = alignment.powf(23.0) *  23.0;
        return format!("{} {} {}",
            (brightness + glow_r).min(255.0) as i32,
            (brightness + glow_g).min(255.0) as i32,
            (brightness + glow_b).min(255.0) as i32,
        );
    }
    let sphere=closest_sphere.unwrap();
    let hitpoint:Vec3=ray.origin.unwrap()+closest_t*ray.direction.unwrap();
    let normal:Vec3=(hitpoint-sphere.centre.unwrap()).unit();
    if sphere.red == Some(255) && sphere.green == Some(220) && sphere.blue == Some(50) {
        let view_dir = (-ray.direction.unwrap()).unit();
        let limb = normal.dot(view_dir).max(0.0);
        let g = (15.0 + 235.0 * limb.powf(0.4)) as i32;
        let b = (180.0 * limb.powf(1.6)) as i32;
        return format!("{} {} {}", 255, g.min(255), b.min(255));
    }
    let to_light  = sun_pos - hitpoint;
    let light_dist = to_light.length();
    let light_dir  = to_light / light_dist;


    let diffuse = normal.dot(light_dir).max(0.0);
    let view_dir = (-ray.direction.unwrap()).unit();
    let reflect  = -light_dir - normal * (2.0 * (-light_dir).dot(normal));
    let spec     = view_dir.dot(reflect).max(0.0).powf(48.0) * 0.85;
    let bright   = 0.05 + diffuse;

    let r = ((sphere.red.unwrap()   as f64 * bright) + spec * 255.0).min(255.0) as i32;
    let g = ((sphere.green.unwrap() as f64 * bright) + spec * 255.0).min(255.0) as i32;
    let b = ((sphere.blue.unwrap()  as f64 * bright) + spec * 255.0).min(255.0) as i32;
    format!("{} {} {}", r, g, b)
}
fn main(){
    let file = File::create("rendered.ppm").unwrap();
    let mut writer = BufWriter::new(file);
    const WIDTH: usize = 8000;
    const HEIGHT: usize = 4500;
    writeln!(writer, "P3\n{} {}\n255\n", WIDTH, HEIGHT).expect("header failed");
    let aspect_ratio: f64 = WIDTH as f64 / HEIGHT as f64;
    let viewport_height: f64 = 2.0;
    let viewport_width: f64 = aspect_ratio * viewport_height;
    let focal_length: f64 = 1.0;
    let horizontal: Vec3 = Vec3::new(viewport_width, 0.0, 0.0);
    let vertical: Vec3 = Vec3::new(0.0, viewport_height, 0.0);
    let origin: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    let lower_left_corner: Vec3 = origin - horizontal / 2.0 - vertical / 2.0 - Vec3::new (0.0, 0.0, focal_length);
    for j in (0..HEIGHT).rev(){
        for i in 0..WIDTH{
            let u:f64=i as f64 /(WIDTH as f64 -1.0);
            let v:f64=j as f64 /(HEIGHT as f64 -1.0);
            let direction:Vec3=lower_left_corner+(u*horizontal)+(v*vertical)-origin;
            let curray=Ray{
                origin:Some(origin),
                direction:Some(direction),
            };
            write_ppm(&mut writer, ray_color(&curray));
        }
    }  
    drop(writer);
    let output = Command::new("magick")
        .arg("rendered.ppm")
        .arg("rendered.png")
        .output()
        .expect("Failed to convert");
    if output.status.success() {
        println!("rendered.png created");
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        println!("Conversion failed: {}", err);
    }
    Command::new("rm")
        .arg("rendered.ppm")
        .output()
        .expect("Failed to remove");
}