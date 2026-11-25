use glow::HasContext;
use std::{collections::HashMap, io::Cursor};

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

impl Rect {
    pub fn inset(self, n: f32) -> Self {
        rect(self.x + n, self.y + n, self.w - 2.0 * n, self.h - 2.0 * n)
    }
    pub fn contains(self, (x, y): (f32, f32)) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    pub fn fit(self, w: f32, h: f32) -> Self {
        let s = (self.w / w).min(self.h / h);
        rect(
            self.x + (self.w - w * s) * 0.5,
            self.y + (self.h - h * s) * 0.5,
            w * s,
            h * s,
        )
    }
}

#[derive(Clone, Copy)]
pub struct Ink(pub u8, pub u8, pub u8, pub u8);

pub const fn rgb(r: u8, g: u8, b: u8) -> Ink {
    Ink(r, g, b, 255)
}

impl Ink {
    pub fn alpha(self, a: u8) -> Self {
        Ink(self.0, self.1, self.2, a)
    }
    fn channels(self) -> [f32; 4] {
        [self.0, self.1, self.2, self.3].map(|v| f32::from(v) / 255.0)
    }
}

pub const WHITE: Ink = rgb(255, 255, 255);
pub const BODY: usize = 0;
pub const DISPLAY: usize = 1;

#[derive(Clone, Copy, Default)]
struct Glyph {
    region: Rect,
    left: f32,
    top: f32,
    advance: f32,
}

struct Atlas {
    sprites: HashMap<&'static str, Rect>,
    glyphs: [[Glyph; 95]; 2],
    font_sizes: [f32; 2],
    size: (f32, f32),
    solid: [f32; 2],
}

impl Atlas {
    fn load(size: (u32, u32)) -> Self {
        let mut atlas = Self {
            sprites: HashMap::new(),
            glyphs: [[Glyph::default(); 95]; 2],
            font_sizes: [0.0; 2],
            size: (size.0 as f32, size.1 as f32),
            solid: [0.0; 2],
        };
        // These coordinates and pixels are baked and embedded together.
        for line in include_str!("../assets/raster/atlas.txt").lines() {
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            let number = |i: usize| fields[i].parse::<f32>().expect("baked atlas number");
            let region = |i| rect(number(i), number(i + 1), number(i + 2), number(i + 3));
            if fields[0] == "font" {
                atlas.font_sizes[fields[1].parse::<usize>().unwrap()] = number(2);
            } else if fields[0] == "s" {
                atlas.sprites.insert(fields[1], region(2));
            } else {
                let face: usize = fields[1].parse().unwrap();
                let code: usize = fields[2].parse().unwrap();
                atlas.glyphs[face][code - 32] = Glyph {
                    region: region(3),
                    left: number(7),
                    top: number(8),
                    advance: number(9),
                };
            }
        }
        let white = atlas.sprites["white"];
        atlas.solid = [
            (white.x + 2.0) / atlas.size.0,
            (white.y + 2.0) / atlas.size.1,
        ];
        atlas
    }
    fn glyph(&self, face: usize, ch: char) -> Glyph {
        let index = if (' '..='~').contains(&ch) {
            ch as usize
        } else {
            b'?' as usize
        };
        self.glyphs[face][index - 32]
    }
}

#[derive(Clone, Copy)]
struct Transform {
    x: f32,
    y: f32,
    scale: f32,
}

pub struct Canvas {
    atlas: Atlas,
    vertices: Vec<[f32; 8]>,
    transform: Transform,
    pub w: f32,
    pub h: f32,
}

impl Canvas {
    fn new(atlas: Atlas) -> Self {
        Self {
            atlas,
            vertices: Vec::with_capacity(40000),
            transform: Transform {
                x: 0.0,
                y: 0.0,
                scale: 1.0,
            },
            w: 1280.0,
            h: 720.0,
        }
    }
    pub fn begin(&mut self, width: u32, height: u32) {
        let (w, h) = (width.max(1) as f32, height.max(1) as f32);
        let scale = (w / 640.0).min(h / 720.0);
        self.w = w / scale;
        self.h = h / scale;
        self.vertices.clear();
    }
    pub fn world(&self, r: Rect) -> Rect {
        let t = self.transform;
        rect(
            t.x + r.x * t.scale,
            t.y + r.y * t.scale,
            r.w * t.scale,
            r.h * t.scale,
        )
    }
    pub fn within(&mut self, dest: Rect, width: f32, height: f32, draw: impl FnOnce(&mut Self)) {
        let fitted = self.world(dest.fit(width, height));
        let old = self.transform;
        self.transform = Transform {
            x: fitted.x,
            y: fitted.y,
            scale: fitted.w / width,
        };
        draw(self);
        self.transform = old;
    }
    fn polygon(&mut self, points: &[[f32; 2]], ink: Ink) {
        let t = self.transform;
        let [u, v] = self.atlas.solid;
        let [r, g, b, a] = ink.channels();
        let vertex = |[x, y]: [f32; 2]| [t.x + x * t.scale, t.y + y * t.scale, u, v, r, g, b, a];
        let first = vertex(points[0]);
        for edge in points[1..].windows(2) {
            self.vertices
                .extend([first, vertex(edge[0]), vertex(edge[1])]);
        }
    }
    pub fn fill(&mut self, r: Rect, c: Ink) {
        self.polygon(
            &[
                [r.x, r.y],
                [r.x + r.w, r.y],
                [r.x + r.w, r.y + r.h],
                [r.x, r.y + r.h],
            ],
            c,
        );
    }
    pub fn round(&mut self, r: Rect, radius: f32, c: Ink) {
        let radius = radius.min(r.w * 0.5).min(r.h * 0.5);
        let mut points = [[0.0; 2]; 24];
        for (i, (x, y)) in [
            (r.x + r.w - radius, r.y + radius),
            (r.x + r.w - radius, r.y + r.h - radius),
            (r.x + radius, r.y + r.h - radius),
            (r.x + radius, r.y + radius),
        ]
        .into_iter()
        .enumerate()
        {
            for k in 0..6 {
                let angle = (i as f32 - 1.0 + k as f32 / 5.0) * std::f32::consts::FRAC_PI_2;
                points[i * 6 + k] = [x + radius * angle.cos(), y + radius * angle.sin()];
            }
        }
        self.polygon(&points, c);
    }
    pub fn ellipse(&mut self, r: Rect, c: Ink) {
        let mut points = [[0.0; 2]; 48];
        for (i, p) in points.iter_mut().enumerate() {
            let a = i as f32 * std::f32::consts::TAU / 48.0;
            *p = [
                r.x + (1.0 + a.cos()) * r.w * 0.5,
                r.y + (1.0 + a.sin()) * r.h * 0.5,
            ];
        }
        self.polygon(&points, c);
    }
    pub fn star(&mut self, x: f32, y: f32, r: f32, c: Ink) {
        for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            self.polygon(
                &[
                    [x, y],
                    [x + r * dx, y],
                    [x + r * 0.23 * dx, y + r * 0.23 * dy],
                    [x, y + r * dy],
                ],
                c,
            );
        }
    }
    fn blit(&mut self, src: Rect, dest: Rect, turns: usize, tint: Ink) {
        let d = self.world(dest);
        let (aw, ah) = self.atlas.size;
        let (u, v, u2, v2) = (
            src.x / aw,
            src.y / ah,
            (src.x + src.w) / aw,
            (src.y + src.h) / ah,
        );
        let tex = [[u, v], [u2, v], [u2, v2], [u, v2]];
        let [r, g, b, a] = tint.channels();
        let mut corners = [[0.0; 8]; 4];
        for (i, [x, y]) in [
            [d.x, d.y],
            [d.x + d.w, d.y],
            [d.x + d.w, d.y + d.h],
            [d.x, d.y + d.h],
        ]
        .into_iter()
        .enumerate()
        {
            let [u, v] = tex[(i + 4 - turns % 4) % 4];
            corners[i] = [x, y, u, v, r, g, b, a];
        }
        for index in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(corners[index]);
        }
    }
    pub fn tile(&mut self, name: &str, dest: Rect, turns: usize, tint: Ink) {
        self.blit(self.atlas.sprites[name], dest, turns, tint);
    }
    pub fn art(&mut self, name: &str, area: Rect) {
        let src = self.atlas.sprites[name];
        self.blit(src, area.fit(src.w, src.h), 0, WHITE);
    }
    pub fn text_width(&self, face: usize, size: f32, text: &str) -> f32 {
        text.chars()
            .map(|c| self.atlas.glyph(face, c).advance)
            .sum::<f32>()
            * size
            / self.atlas.font_sizes[face]
    }
    // y is the text baseline.
    pub fn text(&mut self, face: usize, size: f32, mut x: f32, y: f32, text: &str, ink: Ink) {
        let scale = size / self.atlas.font_sizes[face];
        for ch in text.chars() {
            let g = self.atlas.glyph(face, ch);
            if ch != ' ' {
                self.blit(
                    g.region,
                    rect(
                        x + g.left * scale,
                        y + g.top * scale,
                        g.region.w * scale,
                        g.region.h * scale,
                    ),
                    0,
                    ink,
                );
            }
            x += g.advance * scale;
        }
    }
    pub fn center_text(&mut self, face: usize, size: f32, x: f32, y: f32, text: &str, ink: Ink) {
        self.text(
            face,
            size,
            x - self.text_width(face, size, text) * 0.5,
            y,
            text,
            ink,
        );
    }
    pub fn right_text(&mut self, face: usize, size: f32, x: f32, y: f32, text: &str, ink: Ink) {
        self.text(
            face,
            size,
            x - self.text_width(face, size, text),
            y,
            text,
            ink,
        );
    }
}

pub struct Renderer {
    gl: glow::Context,
    context_valid: bool,
    program: glow::Program,
    vertex_array: Option<glow::VertexArray>,
    buffer: glow::Buffer,
    texture: glow::Texture,
    resolution: glow::UniformLocation,
    pub canvas: Canvas,
}

const VERTEX: &str = r#"
#if __VERSION__ >= 150
#define ATTR in
out vec2 v_uv;
out vec4 v_tint;
#else
#define ATTR attribute
varying mediump vec2 v_uv;
varying lowp vec4 v_tint;
#endif
ATTR vec2 position;
ATTR vec2 uv;
ATTR vec4 tint;
uniform vec2 resolution;
void main() {
    vec2 clip = position / resolution * 2.0 - 1.0;
    gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
    v_uv = uv;
    v_tint = tint;
}"#;

const FRAGMENT: &str = r#"
#ifdef GL_ES
precision mediump float;
#endif
#if __VERSION__ >= 150
in vec2 v_uv;
in vec4 v_tint;
out vec4 pixel;
#define SAMPLE texture
#else
varying mediump vec2 v_uv;
varying lowp vec4 v_tint;
#define pixel gl_FragColor
#define SAMPLE texture2D
#endif
uniform sampler2D atlas;
void main() {
    vec4 sample_color = SAMPLE(atlas, v_uv);
    pixel = vec4(sample_color.rgb * v_tint.rgb * v_tint.a, sample_color.a * v_tint.a);
}"#;

impl Renderer {
    // The caller makes its GL context current for rendering and cleanup.
    pub fn new(gl: glow::Context) -> Result<Self, String> {
        let mut png = png::Decoder::new(Cursor::new(include_bytes!("../assets/raster/atlas.png")))
            .read_info()
            .map_err(|e| e.to_string())?;
        let mut pixels = vec![0; png.output_buffer_size()];
        let info = png.next_frame(&mut pixels).map_err(|e| e.to_string())?;
        if info.color_type != png::ColorType::Rgba {
            return Err("Expected an RGBA sprite atlas".into());
        }
        for p in pixels.chunks_exact_mut(4) {
            let alpha = u16::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u16::from(*c) * alpha + 127) / 255) as u8;
            }
        }
        let atlas = Atlas::load((info.width, info.height));
        unsafe {
            eprintln!(
                "OpenGL: {} ({})",
                gl.get_parameter_string(glow::RENDERER),
                gl.get_parameter_string(glow::VERSION)
            );
            let program = gl.create_program()?;
            let version = gl.version();
            let core_profile = !version.is_embedded
                && (version.major, version.minor) >= (3, 2)
                && gl.get_parameter_i32(glow::CONTEXT_PROFILE_MASK) as u32
                    & glow::CONTEXT_CORE_PROFILE_BIT
                    != 0;
            let shader_header = if version.is_embedded {
                "#version 100\n"
            } else if core_profile {
                "#version 150\n"
            } else {
                "#version 120\n#define mediump\n#define lowp\n"
            };
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX),
                (glow::FRAGMENT_SHADER, FRAGMENT),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, &format!("{shader_header}{source}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            for (i, name) in ["position", "uv", "tint"].iter().enumerate() {
                gl.bind_attrib_location(program, i as u32, name);
            }
            if core_profile {
                gl.bind_frag_data_location(program, 0, "pixel");
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            gl.use_program(Some(program));
            let resolution = gl
                .get_uniform_location(program, "resolution")
                .ok_or("Missing shader resolution")?;
            gl.uniform_1_i32(gl.get_uniform_location(program, "atlas").as_ref(), 0);
            let vertex_array = if core_profile {
                Some(gl.create_vertex_array()?)
            } else {
                None
            };
            let buffer = gl.create_buffer()?;
            let texture = gl.create_texture()?;
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (key, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR_MIPMAP_LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, key, value as i32);
            }
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                info.width as i32,
                info.height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&pixels)),
            );
            gl.generate_mipmap(glow::TEXTURE_2D);
            let error = gl.get_error();
            if error != glow::NO_ERROR {
                return Err(format!("OpenGL atlas setup failed (0x{error:04x})"));
            }
            Ok(Self {
                gl,
                context_valid: true,
                program,
                vertex_array,
                buffer,
                texture,
                resolution,
                canvas: Canvas::new(atlas),
            })
        }
    }

    pub fn bind_framebuffer(&self, name: u32) {
        let fbo = std::num::NonZeroU32::new(name).map(glow::NativeFramebuffer);
        unsafe { self.gl.bind_framebuffer(glow::FRAMEBUFFER, fbo) };
    }

    /// A lost context already owns and discards its GL objects.
    pub fn abandon(mut self) {
        self.context_valid = false;
    }

    pub fn present(&self, width: u32, height: u32) {
        unsafe {
            let gl = &self.gl;
            gl.use_program(Some(self.program));
            if let Some(array) = self.vertex_array {
                gl.bind_vertex_array(Some(array));
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer));
            for (index, n, offset) in [(0, 2, 0), (1, 2, 8), (2, 4, 16)] {
                gl.enable_vertex_attrib_array(index);
                gl.vertex_attrib_pointer_f32(index, n, glow::FLOAT, false, 32, offset);
            }
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            for cap in [
                glow::DEPTH_TEST,
                glow::STENCIL_TEST,
                glow::CULL_FACE,
                glow::SCISSOR_TEST,
            ] {
                gl.disable(cap);
            }
            gl.color_mask(true, true, true, true);
            gl.enable(glow::BLEND);
            gl.blend_equation(glow::FUNC_ADD);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl
                .uniform_2_f32(Some(&self.resolution), self.canvas.w, self.canvas.h);
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&self.canvas.vertices),
                glow::STREAM_DRAW,
            );
            self.gl
                .draw_arrays(glow::TRIANGLES, 0, self.canvas.vertices.len() as i32);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if !self.context_valid {
            return;
        }
        unsafe {
            self.gl.delete_texture(self.texture);
            if let Some(array) = self.vertex_array {
                self.gl.delete_vertex_array(array);
            }
            self.gl.delete_buffer(self.buffer);
            self.gl.delete_program(self.program);
        }
    }
}
