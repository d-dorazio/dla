use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time;

use structopt::StructOpt;

use dla::{Dla, Vec3};

/// Simulate 3D diffusion limited aggregation (DLA for short) and save the final
/// system as a scene ready to be rendered using povray for example.
#[derive(StructOpt, Debug)]
struct App {
    /// Number of particles to add to the DLA system.
    #[structopt(short = "p", long = "particles", default_value = "10000")]
    particles: usize,

    /// The attraction radius of each particle that makes other particles stick
    /// to it.
    #[structopt(short = "a", long = "attraction-radius", default_value = "8")]
    attraction_radius: u16,

    /// How far away new particles are generated from the core of the current
    /// DLA.
    #[structopt(short = "g", long = "spawn-radius", default_value = "10")]
    spawn_radius: u32,

    /// The output formats the scene should be saved as. As of now `javascript,
    /// `povray` and `csv` are supported.
    #[structopt(short = "s", long = "scene-format", default_value = "povray")]
    scene_formats: Vec<SceneFormat>,

    /// Output filename where to save the scene.
    #[structopt(parse(from_os_str), default_value = "dla.pov")]
    output: PathBuf,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum SceneFormat {
    Povray,
    Js,
    Csv,
}

#[derive(Debug)]
struct Scene {
    camera: Camera,
    lights: Vec<Light>,
    dla: Dla,
}

#[derive(Debug)]
struct Camera {
    position: Vec3,
    target: Vec3,
}

#[derive(Debug)]
struct Light {
    position: Vec3,
    intensity: f32,
}

fn main() -> io::Result<()> {
    let args = App::from_args();

    let seeds = vec![Vec3::new(0, 0, 0)];
    let mut dla = Dla::new(args.spawn_radius, args.attraction_radius, seeds).unwrap();

    let start = time::Instant::now();
    let mut rng = rand::thread_rng();
    for i in 0..args.particles {
        if i % 100 == 0 {
            print!("\rgenerated {} particles, progress: {}%", i, i * 100 / args.particles);
            io::stdout().flush()?;
        }

        dla.add(&mut rng);
    }

    // clear progress line manually to not install another dep
    println!(
        "\r          {:width$}                          ",
        " ",
        width = args.particles.to_string().len()
    );

    let duration = start.elapsed();

    #[rustfmt::skip]
    println!(
        r#"# DLA

The DLA system was correctly generated in {}m {}s.

It contains {} particles and its bounding box goes from
({},{},{}) to ({},{},{}) with a total volume of {}.
"#,
        duration.as_secs() / 60, duration.as_secs() % 60,
        dla.len(),
        dla.bbox().lower().x, dla.bbox().lower().y, dla.bbox().lower().z,
        dla.bbox().upper().x, dla.bbox().upper().y, dla.bbox().upper().z,
        dla.bbox().volume(),
    );

    let scene = Scene::new(dla);

    let scene_formats = args.scene_formats.into_iter().collect::<HashSet<_>>();

    for r in scene_formats {
        match r {
            SceneFormat::Povray => save_pov_scene(&args.output, &scene)?,
            SceneFormat::Js => save_js_scene(&args.output, &scene)?,
            SceneFormat::Csv => save_csv_scene(&args.output, &scene)?,
        }
    }

    Ok(())
}

fn save_pov_scene(path: &PathBuf, Scene { dla, camera, lights }: &Scene) -> io::Result<()> {
    let path = path.with_extension("pov");
    let mut out = BufWriter::new(File::create(&path)?);

    let bbox = dla.bbox();

    #[rustfmt::skip]
    writeln!(
        out,
        r#"// 3D DLA geometry - generated by github.com/d-dorazio/dla

#version 3.7;

#include "colors.inc"

global_settings {{ assumed_gamma 1.0 }}
#default{{ finish {{ ambient 0.1 diffuse 0.9 }} }}

background {{ color Black }}

// scene bbox <{}, {}, {}> <{}, {}, {}>

camera {{
  location <{}, {}, {}>
  look_at <{}, {}, {}>
}}
"#,
        bbox.lower().x, bbox.lower().y, bbox.lower().z,
        bbox.upper().x, bbox.upper().y, bbox.upper().z,
        camera.position.x, camera.position.y, camera.position.z,
        camera.target.x, camera.target.y, camera.target.z,
    )?;

    for light in lights {
        #[rustfmt::skip]
        writeln!(
            out,
            "light_source {{ <{}, {}, {}> color rgb <{}, {}, {}> }}",
            light.position.x, light.position.y, light.position.z,
            light.intensity, light.intensity, light.intensity
        )?;
    }

    let center = bbox.center();
    let mut cells = dla.cells().map(|cc| (cc, center.dist2(*cc))).collect::<Vec<_>>();
    cells.sort_by_key(|(_, d)| *d);

    let max_d = cells.last().expect("empty dla, cannot happen since it should be seeded").1;
    let mut cells = cells.into_iter();

    let gradients = 3;
    let n = gradients * 2;
    for i in 0..n {
        writeln!(out, "\nunion {{")?;
        for (p, _) in cells.by_ref().take_while(|(_, dd)| *dd <= (i + 1) * max_d / n) {
            writeln!(out, "  sphere {{ <{}, {}, {}>, 1 }}", p.x, p.y, p.z)?;
        }

        let (r, g, b) = match 5 + i / gradients {
            0..=2 => (0.27, 0.3, 0.02),
            3..=4 => (0.0, 0.6, 0.02),
            5 => (0.34, 0.7, 0.03),
            6 => (0.85, 0.84, 0.00),
            _ => unreachable!(),
        };

        let f = (1.0 + (i % gradients) as f64) / (gradients as f64);
        let (r, g, b) = (r * f, g * f, b * f);

        writeln!(
            out,
            r#"  texture {{
    pigment {{ color rgb<{}, {}, {}> }}
    finish {{ phong 0.5 }}
  }}
}}"#,
            r, g, b
        )?;
    }

    println!(
        r#"## PovRay Scene

The DLA scene has been saved as a PovRay scene ({path}) which is possible to
render with a command like the following

`povray +A +W1600 +H1600 {path}`
"#,
        path = path.display()
    );

    Ok(())
}

fn save_js_scene(path: &PathBuf, Scene { dla, camera, lights }: &Scene) -> io::Result<()> {
    let path = path.with_extension("js");
    let mut out = BufWriter::new(File::create(&path)?);

    let scene_bbox = dla.bbox();

    #[rustfmt::skip]
    writeln!(
        out,
        r#"// 3D DLA geometry - generated by github.com/d-dorazio/dla

var DLA = {{
    bbox: {{
        lower: {{ x: {}, y: {}, z: {} }},
        upper: {{ x: {}, y: {}, z: {} }},
    }},
    camera: {{
        position: {{ x: {}, y: {}, z: {} }},
        look_at: {{ x: {}, y: {}, z: {} }},
    }},
    lights: ["#,
        scene_bbox.lower().x, scene_bbox.lower().y, scene_bbox.lower().z,
        scene_bbox.upper().x, scene_bbox.upper().y, scene_bbox.upper().z,
        camera.position.x, camera.position.y, camera.position.z,
        camera.target.x, camera.target.y, camera.target.z,
    )?;

    for light in lights {
        writeln!(
            out,
            "        {{ position: {{ x: {}, y: {}, z: {} }}, intensity: {} }},",
            light.position.x, light.position.y, light.position.z, light.intensity
        )?;
    }

    writeln!(
        out,
        r#"    ],
    particles: ["#
    )?;

    for p in dla.cells() {
        writeln!(out, "        {{ x: {}, y: {}, z: {} }},", p.x, p.y, p.z)?;
    }

    writeln!(
        out,
        r#"    ],
}};"#
    )?;

    println!(
        r#"## Javascript Scene

The DLA scene has been saved as a Javascript file ({path}) that contains a
single object `DLA` that has the `particles` alongside a `camera` and `lights`.
"#,
        path = path.display()
    );

    Ok(())
}

fn save_csv_scene(path: &PathBuf, Scene { dla, .. }: &Scene) -> io::Result<()> {
    let path = path.with_extension("csv");
    let mut out = BufWriter::new(File::create(&path)?);

    for c in dla.cells() {
        writeln!(out, "{},{},{}", c.x, c.y, c.z)?;
    }

    println!(
        r#"## Csv Scene

The positions (x,y,z) of all the cells that form the DLA have been saved as a CSV file ({path}).
"#,
        path = path.display()
    );

    Ok(())
}

impl Scene {
    /// build a scene from a DLA with camera and lights in a completely
    /// arbitrary way.
    fn new(dla: Dla) -> Self {
        let scene_bbox = dla.bbox();
        let scene_dimensions = scene_bbox.dimensions();
        let away_dist = scene_dimensions.x.min(scene_dimensions.y).min(scene_dimensions.z);

        let camera = Camera {
            position: Vec3::new(
                scene_bbox.center().x - away_dist,
                scene_bbox.center().y,
                scene_bbox.lower().z - away_dist,
            ),
            target: Vec3::new(0, 0, 0),
        };

        let mut lights = vec![];
        let mut add_light = |pt: Vec3, intensity| {
            let position = pt + (pt - scene_bbox.center()).normalized() * away_dist;
            lights.push(Light { position, intensity })
        };

        // key light
        add_light(
            Vec3::new(scene_bbox.lower().x, scene_bbox.center().y, scene_bbox.lower().z),
            1.0,
        );

        // front light
        add_light(
            Vec3::new(
                scene_bbox.center().x,
                scene_bbox.center().y,
                scene_bbox.lower().z - away_dist / 2,
            ),
            0.2,
        );

        // fill light
        add_light(
            Vec3::new(scene_bbox.upper().x, scene_bbox.lower().y, scene_bbox.lower().z),
            0.75,
        );

        // background light
        add_light(Vec3::new(scene_bbox.lower().x, scene_bbox.upper().y, scene_bbox.upper().z), 0.5);

        // bottom light
        add_light(
            Vec3::new(scene_bbox.center().x, scene_bbox.lower().y, scene_bbox.center().z),
            0.75,
        );

        // top light
        add_light(
            Vec3::new(scene_bbox.center().x, scene_bbox.upper().y, scene_bbox.center().z),
            0.5,
        );

        Scene { camera, lights, dla }
    }
}

impl std::str::FromStr for SceneFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "povray" => Ok(SceneFormat::Povray),
            "javascript" | "js" => Ok(SceneFormat::Js),
            "csv" => Ok(SceneFormat::Csv),
            s => Err(format!("`{}` is not a valid scene format", s)),
        }
    }
}
