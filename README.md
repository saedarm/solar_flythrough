# solar_flythrough

A parallel ray-traced tour of a toy solar system, rendered frame-by-frame in Rust and stitched into an MP4 with ffmpeg. The camera orbits a fixed sun while eight planets trace their own orbits at independent speeds. Every frame is a fresh ray-traced render — no rasterization, no engine, just sphere intersections and a tiny shader. Earth gets procedural Perlin-noise continents, Jupiter gets perturbed bands, the sun gets limb darkening, and the background gets a soft glow that brightens when the camera looks toward the sun.

Output: 300 frames at 1280×720, 30fps, 10 seconds → `out.mp4`. Around 5-15 MB.

## Quick start

```bash
cargo run --release
```

That's it. The release flag is not optional — debug builds run 20-50× slower for this kind of float-heavy math. First run will compile dependencies (rayon, noise, once_cell, image) which takes a couple of minutes, then it'll start churning frames.

You need two things on your PATH:

- **Rust** (stable, 2021 edition) — install from [rustup.rs](https://rustup.rs)
- **ffmpeg** — for the final PNG→MP4 stitch

Verify ffmpeg works before kicking off the full run:

```bash
ffmpeg -version
```

If `ffmpeg` 404s when you call it, install it: `winget install ffmpeg` on Windows, `brew install ffmpeg` on Mac, `sudo apt install ffmpeg` on Debian/Ubuntu. Open a fresh terminal afterward so PATH gets picked up.

PNG writing is handled inside Rust via the `image` crate — no ImageMagick, no temp files, no external tool for per-frame output.

## What it does, step by step

When you run `cargo run --release`, `main` does this:

1. Creates the `frames/` directory if it doesn't exist
2. Loops `frame` from 0 to 299, calling `render_frame(frame)` each time
3. After all frames are written, shells out to ffmpeg to stitch them into `out.mp4`

Each `render_frame` call:

1. Checks if `frames/frame_NNNN.png` already exists — if so, skips. This makes the whole thing resumable: ctrl-C and restart, it picks up where it left off
2. Computes `t = frame / FPS` (wall-clock time for orbits) and `t_norm = frame / TOTAL_FRAMES` (0..1 for the camera path)
3. Builds the scene at time `t` (sun at fixed position, planets on circular orbits)
4. Places the camera on its orbit at `t_norm` and builds a look-at basis pointing at the sun
5. Parallel-renders every row of the image across all CPU cores via Rayon, collecting `Vec<[u8; 3]>` rows
6. Flattens the rows into a single byte buffer and writes the PNG via `image::save_buffer`

While it's running, you can `ls frames/` in another terminal and watch PNGs accumulate. The `println!("frame N/300 done")` lines are your progress bar. Or open `frames/` in a file explorer with thumbnails on — you can literally see the camera moving as frames land.

## Expected timing and disk usage

Roughly, on a modern multi-core laptop:

- 1280×720 (default): roughly 1 second per frame, ~5 minutes total
- 1920×1080: 2-5 seconds per frame, 10-25 minutes total
- 640×360 (preview): well under a second per frame, 1-2 minutes total

Disk during the run: ~100-200 MB of PNGs in `frames/` at 720p. After ffmpeg makes `out.mp4`, you can `rm -rf frames/` to reclaim that. The MP4 itself is 5-15 MB.

## Iterate fast first

Before committing to a full render, drop the resolution and duration at the top of `main.rs` for a fast sanity check:

```rust
const WIDTH: usize = 320;
const HEIGHT: usize = 180;
const FPS: u32 = 30;
const DURATION_SECS: u32 = 2;   // 60 frames, ~30 seconds total render
```

Get the framing and orbit looking right, then crank everything back up:

```rust
const WIDTH: usize = 1280;
const HEIGHT: usize = 720;
const FPS: u32 = 30;
const DURATION_SECS: u32 = 10;
```

Remember to `rm -rf frames/` (or delete the folder manually) between resolution changes — otherwise the resume logic will keep the old low-res frames and your output will be a mix of resolutions, which ffmpeg won't like.

## Re-stitching without re-rendering

If you just want to tweak ffmpeg flags, don't re-run the whole thing. Run ffmpeg directly against the existing PNGs:

```bash
# Same flags the Rust code uses
ffmpeg -y -framerate 30 -i frames/frame_%04d.png -c:v libx264 -pix_fmt yuv420p -crf 18 out.mp4

# Make a GIF for embedding in a blog post
ffmpeg -i out.mp4 -vf "fps=15,scale=720:-1:flags=lanczos" out.gif

# Higher quality MP4 (slower encode, smaller file at same quality)
ffmpeg -y -framerate 30 -i frames/frame_%04d.png -c:v libx264 -pix_fmt yuv420p -crf 14 -preset slow out.mp4
```

## Project layout

```
solar_flythrough/
├── Cargo.toml           # rayon, noise, once_cell, image + release profile tuning
├── Cargo.lock           # generated on first build
├── .gitignore
├── README.md
├── src/
│   └── main.rs          # everything: vec math, scene, shader, frame loop
├── frames/              # generated at runtime, gitignored
│   ├── frame_0000.png
│   └── ...
├── out.mp4              # final output, gitignored
└── target/              # cargo build artifacts, gitignored
```

## What's in main.rs

- **Vec3** — operator overloads for arithmetic plus `dot`, `cross`, `unit`, `length`. The `cross` product is what makes the look-at camera work.
- **Sphere + Material enum** — material tags the sphere as Sun, Earth, Jupiter, or generic Solid, so the shader can pick the right code path without RGB-equality hacks.
- **hits** — standard ray-sphere intersection. Returns the nearest positive `t` or `-1.0` for a miss.
- **EARTH_NOISE / JUPITER_NOISE** — fractional Brownian motion stacked on Perlin, built once via `once_cell::sync::Lazy` so all threads share the same generator without contention. Different seeds (42 vs 7) so the two planets don't share a pattern.
- **earth_color** — samples 3D noise at the surface normal, thresholds into ocean/water/forest/highlands/snow. Returns RGB. The normal of a sphere is already the unit-sphere coordinate of the hit point, so it doubles as the noise input — the texture is scale-invariant.
- **jupiter_color** — uses latitude (y component of normal) to make horizontal bands via `sin(lat * 8)`, then perturbs latitude with noise so the band edges become wavy and turbulent instead of perfect horizontal lines.
- **build_scene(t)** — sun at `(0, 0, -8)` with radius 1.5, then eight planets on circular orbits in the xz plane with a small y offset per planet. Speed column controls how fast each planet revolves; `t` is wall-clock seconds.
- **camera_at(t_norm)** — camera position on a circle of radius 15 around the sun, slightly above the orbital plane. One full revolution over the duration of the clip.
- **ray_color** — sphere hit goes through Sun → limb darkening, Earth → noise texture + Phong shading, Jupiter → band texture + Phong shading, everything else → flat color + Phong. Miss returns the starfield with a sun glow term. Returns `[u8; 3]` for direct insertion into the PNG buffer.
- **render_frame** — builds the scene, constructs the camera basis with `forward × world_up = right`, then `right × forward = up`, sets up the viewport, parallel-renders rows via `into_par_iter().rev()`, flattens to a byte buffer, saves PNG via `image::save_buffer`.
- **main** — `fs::create_dir_all("frames")`, render loop, ffmpeg.

## Tuning knobs

In `camera_at`:

- `orbit_radius` — distance from sun. Smaller = sun looms larger. Try 8 for dramatic, 20 for distant.
- `orbit_height` — vertical offset. `0.0` gives an edge-on view where planets sweep across the sun's disc. `5.0+` is more top-down.
- Replace `t_norm * TAU` with `t_norm * PI` for a half-orbit, or with a smoothstep `t_norm * t_norm * (3.0 - 2.0 * t_norm) * TAU` for easing.

In `build_scene`:

- Planet table is `(orbit_radius, orbit_speed, y_offset, body_radius, r, g, b, material)`.
- Speeds are radians per wall-clock second. At defaults, Mercury laps the sun ~2.5 times in 10 seconds while Neptune barely moves. Scale the column by 0.3 to slow everything; multiply by 3 to make inner planets blur.

In `earth_color` thresholds:

- Perlin output clusters near zero, so the thresholds aren't uniform. To get more land, lower `-0.05` and `0.05`. To get a more icy world, push the snow threshold down from `0.5`.

In `jupiter_color`:

- `lat * 8.0` is the band count. Lower = fewer thicker bands; higher = more bands.
- The `0.15` turbulence coefficient is how wavy the band edges get. Push to `0.3` for storm cells.

## Going further

Stuff worth trying once the basic version is rendering cleanly:

- **Rotating planets** — currently the texture is locked to world space, so as the planet orbits, the pattern "swims" across it. Fix: store a rotation angle per planet and rotate the normal into the planet's local frame before sampling noise.
- **Saturn rings** — ray vs disk-segment intersection in Saturn's local frame. Accept hits where `inner_radius < dist < outer_radius` and the ray crosses the ring plane.
- **Asteroid belt** — generate ~300 tiny random spheres between Mars and Jupiter at scene-build time, seeded so positions are stable per-frame. Your existing hit loop handles them for free.
- **Audio-reactive** — pre-analyze a Suno track's amplitude per frame, drive `orbit_radius` or sun brightness from the envelope, render a music video.
- **Drop ffmpeg too** — the `mp4` or `re_mp4` crates can encode H.264 in pure Rust, removing the last external dependency. Trickier than `image::save_buffer` but doable.

## License

MIT.
