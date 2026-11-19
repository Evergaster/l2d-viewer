//! Rasterizador por software, mínimo y honesto: triángulos con coordenadas de textura,
//! interpolación baricéntrica y blending por alpha.
//!
//! Existe para poder **ver** datos sin depender de una GPU y para que el pipeline de
//! dibujo sea comprobable con pruebas. La versión con `wgpu` llega en la fase 3.
//!
//! Sin antialias, sin mipmaps, sin perspectiva (los modelos Live2D son 2D con
//! transformaciones afines por malla, así que la interpolación lineal es suficiente).

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Normal,
    Additive,
    Multiplicative,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Posición en píxeles de canvas (origen arriba-izquierda).
    pub pos: [f32; 2],
    /// Coordenada de textura normalizada (0..1).
    pub uv: [f32; 2],
    /// Color RGBA multiplicativo (1,1,1,1 = sin efecto).
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(x: f32, y: f32, u: f32, v: f32) -> Self {
        Self {
            pos: [x, y],
            uv: [u, v],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }
}

#[derive(Debug, Clone)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    /// RGBA8, fila mayor, sin premultiplicar.
    pub pixels: Vec<u8>,
}

impl Texture {
    /// Carga un PNG y lo convierte a RGBA8. Soporta RGB, RGBA, gris y gris+alfa.
    pub fn from_png(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut decoder = png::Decoder::new(reader);
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info()?;
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf)?;
        let bytes = &buf[..info.buffer_size()];
        let (w, h) = (info.width, info.height);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        match info.color_type {
            png::ColorType::Rgba => px.extend_from_slice(bytes),
            png::ColorType::Rgb => {
                for c in bytes.chunks_exact(3) {
                    px.extend_from_slice(&[c[0], c[1], c[2], 255]);
                }
            }
            png::ColorType::Grayscale => {
                for &g in bytes {
                    px.extend_from_slice(&[g, g, g, 255]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for c in bytes.chunks_exact(2) {
                    px.extend_from_slice(&[c[0], c[0], c[0], c[1]]);
                }
            }
            png::ColorType::Indexed => return Err("PNG indexado: no soportado".into()),
        }
        Ok(Texture::from_rgba(w, h, px))
    }

    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        assert_eq!(
            pixels.len(),
            (width * height * 4) as usize,
            "tamaño de textura"
        );
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Damero de dos tonos, para pruebas y para el demo.
    pub fn checker(size: u32, cell: u32, a: [u8; 4], b: [u8; 4]) -> Self {
        let mut t = Texture::new(size, size);
        for y in 0..size {
            for x in 0..size {
                let on = ((x / cell) + (y / cell)) % 2 == 0;
                let c = if on { a } else { b };
                let i = ((y * size + x) * 4) as usize;
                t.pixels[i..i + 4].copy_from_slice(&c);
            }
        }
        t
    }

    /// Muestreo bilineal con clamp en los bordes.
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = (u.clamp(0.0, 1.0) * (self.width.saturating_sub(1)) as f32).max(0.0);
        let y = (v.clamp(0.0, 1.0) * (self.height.saturating_sub(1)) as f32).max(0.0);
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(self.width.saturating_sub(1));
        let y1 = (y0 + 1).min(self.height.saturating_sub(1));
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;

        let px = |px: u32, py: u32| -> [f32; 4] {
            let i = ((py * self.width + px) * 4) as usize;
            [
                self.pixels[i] as f32 / 255.0,
                self.pixels[i + 1] as f32 / 255.0,
                self.pixels[i + 2] as f32 / 255.0,
                self.pixels[i + 3] as f32 / 255.0,
            ]
        };

        let c00 = px(x0, y0);
        let c10 = px(x1, y0);
        let c01 = px(x0, y1);
        let c11 = px(x1, y1);

        let mut out = [0.0f32; 4];
        for k in 0..4 {
            let top = c00[k] * (1.0 - fx) + c10[k] * fx;
            let bot = c01[k] * (1.0 - fx) + c11[k] * fx;
            out[k] = top * (1.0 - fy) + bot * fy;
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    /// RGBA8, fila mayor, sin premultiplicar.
    pub pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn clear(&mut self, rgba: [u8; 4]) {
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
    }

    #[inline]
    fn blend_px(&mut self, x: u32, y: u32, src: [f32; 4], mode: Blend) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y * self.width + x) * 4) as usize;
        let dst = [
            self.pixels[i] as f32 / 255.0,
            self.pixels[i + 1] as f32 / 255.0,
            self.pixels[i + 2] as f32 / 255.0,
            self.pixels[i + 3] as f32 / 255.0,
        ];
        let a = src[3].clamp(0.0, 1.0);

        let out = match mode {
            Blend::Normal => [
                src[0] * a + dst[0] * (1.0 - a),
                src[1] * a + dst[1] * (1.0 - a),
                src[2] * a + dst[2] * (1.0 - a),
                a + dst[3] * (1.0 - a),
            ],
            Blend::Additive => [
                dst[0] + src[0] * a,
                dst[1] + src[1] * a,
                dst[2] + src[2] * a,
                (dst[3] + a).min(1.0),
            ],
            Blend::Multiplicative => [
                dst[0] * (src[0] * a + (1.0 - a)),
                dst[1] * (src[1] * a + (1.0 - a)),
                dst[2] * (src[2] * a + (1.0 - a)),
                a + dst[3] * (1.0 - a),
            ],
        };

        for k in 0..4 {
            self.pixels[i + k] = (out[k].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    /// Dibuja un triángulo. Los vértices van en píxeles; el sentido de giro da igual.
    pub fn triangle(&mut self, v: [Vertex; 3], tex: Option<&Texture>, mode: Blend) {
        self.triangle_masked(v, tex, mode, None);
    }

    /// Igual que [`Canvas::triangle`], pero multiplica el alfa por una máscara
    /// (se usa el canal alfa del canvas que se pase).
    pub fn triangle_masked(
        &mut self,
        v: [Vertex; 3],
        tex: Option<&Texture>,
        mode: Blend,
        mask: Option<&Canvas>,
    ) {
        let [a, b, c] = v;
        let area = edge(a.pos, b.pos, c.pos);
        if area.abs() < f32::EPSILON {
            return; // degenerado
        }

        let min_x = a.pos[0].min(b.pos[0]).min(c.pos[0]).floor().max(0.0) as u32;
        let max_x =
            (a.pos[0].max(b.pos[0]).max(c.pos[0]).ceil() as i64).clamp(0, self.width as i64) as u32;
        let min_y = a.pos[1].min(b.pos[1]).min(c.pos[1]).floor().max(0.0) as u32;
        let max_y = (a.pos[1].max(b.pos[1]).max(c.pos[1]).ceil() as i64)
            .clamp(0, self.height as i64) as u32;

        for y in min_y..max_y {
            for x in min_x..max_x {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let w0 = edge(b.pos, c.pos, p) / area;
                let w1 = edge(c.pos, a.pos, p) / area;
                let w2 = edge(a.pos, b.pos, p) / area;
                if w0 < -1e-6 || w1 < -1e-6 || w2 < -1e-6 {
                    continue;
                }

                let u = w0 * a.uv[0] + w1 * b.uv[0] + w2 * c.uv[0];
                let vv = w0 * a.uv[1] + w1 * b.uv[1] + w2 * c.uv[1];
                let mut col = [
                    w0 * a.color[0] + w1 * b.color[0] + w2 * c.color[0],
                    w0 * a.color[1] + w1 * b.color[1] + w2 * c.color[1],
                    w0 * a.color[2] + w1 * b.color[2] + w2 * c.color[2],
                    w0 * a.color[3] + w1 * b.color[3] + w2 * c.color[3],
                ];
                if let Some(t) = tex {
                    let s = t.sample(u, vv);
                    col = [col[0] * s[0], col[1] * s[1], col[2] * s[2], col[3] * s[3]];
                }
                if let Some(m) = mask {
                    col[3] *= m.pixel(x, y)[3] as f32 / 255.0;
                }
                self.blend_px(x, y, col, mode);
            }
        }
    }

    /// Quad como dos triángulos, con UVs (0,0)-(1,1).
    pub fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, tex: Option<&Texture>, mode: Blend) {
        let v00 = Vertex::new(x, y, 0.0, 0.0);
        let v10 = Vertex::new(x + w, y, 1.0, 0.0);
        let v11 = Vertex::new(x + w, y + h, 1.0, 1.0);
        let v01 = Vertex::new(x, y + h, 0.0, 1.0);
        self.triangle([v00, v10, v11], tex, mode);
        self.triangle([v00, v11, v01], tex, mode);
    }

    /// Reduce el lienzo por un factor entero, promediando bloques (supermuestreo).
    ///
    /// El promedio se hace en **alfa premultiplicado**: promediar RGB con alfa recta mezcla el
    /// color de los píxeles transparentes del borde y saca un filo oscuro. Con premultiplicado se
    /// promedia "cuánta luz aporta cada muestra", y el borde queda del color del trazo con alfa
    /// parcial, que es lo que hace un multisample de verdad.
    pub fn reducir(&self, factor: u32) -> Canvas {
        let factor = factor.max(1);
        if factor == 1 {
            return self.clone();
        }
        let w = (self.width / factor).max(1);
        let h = (self.height / factor).max(1);
        let mut out = Canvas::new(w, h);
        let n = (factor * factor) as f32;
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for dy in 0..factor {
                    for dx in 0..factor {
                        let sx = x * factor + dx;
                        let sy = y * factor + dy;
                        if sx >= self.width || sy >= self.height {
                            continue;
                        }
                        let i = ((sy * self.width + sx) * 4) as usize;
                        let a = self.pixels[i + 3] as f32 / 255.0;
                        acc[0] += self.pixels[i] as f32 / 255.0 * a;
                        acc[1] += self.pixels[i + 1] as f32 / 255.0 * a;
                        acc[2] += self.pixels[i + 2] as f32 / 255.0 * a;
                        acc[3] += a;
                    }
                }
                let a = acc[3] / n;
                let o = ((y * w + x) * 4) as usize;
                let rgb = if a > 1e-4 {
                    [acc[0] / n / a, acc[1] / n / a, acc[2] / n / a]
                } else {
                    [0.0, 0.0, 0.0]
                };
                out.pixels[o] = (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8;
                out.pixels[o + 1] = (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8;
                out.pixels[o + 2] = (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8;
                out.pixels[o + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        out
    }

    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::create(path)?;
        let w = std::io::BufWriter::new(file);
        let mut enc = png::Encoder::new(w, self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header()?;
        writer.write_image_data(&self.pixels)?;
        Ok(())
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }
}

#[inline]
fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangulo_cubre_el_centro() {
        let mut c = Canvas::new(32, 32);
        let v = [
            Vertex::new(0.0, 0.0, 0.0, 0.0).with_color(1.0, 0.0, 0.0, 1.0),
            Vertex::new(32.0, 0.0, 1.0, 0.0).with_color(1.0, 0.0, 0.0, 1.0),
            Vertex::new(16.0, 32.0, 0.5, 1.0).with_color(1.0, 0.0, 0.0, 1.0),
        ];
        c.triangle(v, None, Blend::Normal);
        let center = c.pixel(16, 20);
        assert!(center[0] > 240, "rojo en el centro: {center:?}");
        let outside = c.pixel(30, 30);
        assert_eq!(outside[3], 0, "píxel fuera del triángulo: {outside:?}");
    }

    #[test]
    fn quad_texturizado_es_damero() {
        let tex = Texture::checker(8, 4, [255, 0, 0, 255], [0, 0, 255, 255]);
        let mut c = Canvas::new(16, 16);
        c.quad(0.0, 0.0, 16.0, 16.0, Some(&tex), Blend::Normal);
        let tl = c.pixel(2, 2);
        let tr = c.pixel(13, 2);
        assert!(
            tl[0] > tr[0],
            "rojo a la izquierda ({tl:?}) y azul a la derecha ({tr:?})"
        );
        assert!(tr[2] > tl[2]);
    }

    #[test]
    fn alpha_normal_compone() {
        let mut c = Canvas::new(8, 8);
        c.clear([0, 0, 0, 255]);
        let v = [
            Vertex::new(0.0, 0.0, 0.0, 0.0).with_color(1.0, 1.0, 1.0, 0.5),
            Vertex::new(8.0, 0.0, 1.0, 0.0).with_color(1.0, 1.0, 1.0, 0.5),
            Vertex::new(0.0, 8.0, 0.0, 1.0).with_color(1.0, 1.0, 1.0, 0.5),
        ];
        c.triangle(v, None, Blend::Normal);
        let p = c.pixel(1, 1);
        assert!(
            (p[0] as i32 - 128).abs() < 8,
            "gris medio esperado, hay {p:?}"
        );
    }

    #[test]
    fn triangulo_degenerado_no_hace_nada() {
        let mut c = Canvas::new(4, 4);
        let v = [
            Vertex::new(1.0, 1.0, 0.0, 0.0),
            Vertex::new(1.0, 1.0, 0.0, 0.0),
            Vertex::new(1.0, 1.0, 0.0, 0.0),
        ];
        c.triangle(v, None, Blend::Normal);
        assert_eq!(c.pixel(1, 1)[3], 0);
    }
}

#[cfg(test)]
mod pruebas_reducir {
    use super::*;

    #[test]
    fn reducir_uno_no_cambia_nada() {
        let mut c = Canvas::new(4, 4);
        c.clear([10, 20, 30, 255]);
        let r = c.reducir(1);
        assert_eq!(r.pixels, c.pixels);
        assert_eq!((r.width, r.height), (4, 4));
    }

    #[test]
    fn reducir_dos_promedia_alfa() {
        // 1 de 4 muestras opaca: el píxel reducido queda a un cuarto de alfa, blanco
        let mut c = Canvas::new(2, 2);
        c.clear([0, 0, 0, 0]);
        c.pixels[0..4].copy_from_slice(&[255, 255, 255, 255]);
        let r = c.reducir(2);
        assert_eq!((r.width, r.height), (1, 1));
        let p = r.pixel(0, 0);
        assert!((p[3] as i32 - 64).abs() <= 2, "alfa 0.25, hay {p:?}");
        assert!(p[0] > 240, "el color no se oscurece, hay {p:?}");
    }

    /// El promedio premultiplicado no arrastra el RGB de los píxeles transparentes: si el borde
    /// tuviera blanco con alfa 0, promediar a secas dejaría un filo claro.
    #[test]
    fn el_borde_no_arrastra_color_de_lo_transparente() {
        // 2x2: (0,0) rojo opaco; el resto transparente **blanco** (el peor caso: si se promedia
        // el RGB sin pesarlo por alfa, el resultado sale rosa).
        let mut c = Canvas::new(2, 2);
        c.clear([255, 255, 255, 0]);
        c.pixels[0..4].copy_from_slice(&[200, 0, 0, 255]);
        let r = c.reducir(2);
        let p = r.pixel(0, 0);
        assert!(p[0] > 180 && p[1] < 40 && p[2] < 40, "rojo, no rosa: {p:?}");
        assert!((p[3] as i32 - 64).abs() <= 2, "alfa 0.25: {p:?}");
    }

    #[test]
    fn reducir_no_deforma_el_tamano() {
        let c = Canvas::new(7, 5);
        let r = c.reducir(3);
        assert_eq!((r.width, r.height), (2, 1));
    }
}
