use crate::{core::Matrix, ext::get_viewport};
use glyph_brush::{
    ab_glyph::{Font, FontArc, ScaleFont},
    BrushAction, BrushError, GlyphBrush, GlyphBrushBuilder, GlyphCruncher, Layout, Section, Text,
};
use macroquad::{
    miniquad::{Texture, TextureParams},
    prelude::*,
};
use std::borrow::Cow;

use super::Ui;

#[must_use = "DrawText does nothing until you 'draw' it"]
pub struct DrawText<'a, 's, 'ui> {
    pub ui: &'ui mut Ui<'a>,
    text: Option<Cow<'s, str>>,
    size: f32,
    pos: (f32, f32),
    anchor: (f32, f32),
    color: Color,
    max_width: Option<f32>,
    baseline: bool,
    multiline: bool,
    scale: Matrix,
}

impl<'a, 's, 'ui> DrawText<'a, 's, 'ui> {
    pub(crate) fn new(ui: &'ui mut Ui<'a>, text: Cow<'s, str>) -> Self {
        Self {
            ui,
            text: Some(text),
            size: 1.,
            pos: (0., 0.),
            anchor: (0., 0.),
            color: WHITE,
            max_width: None,
            baseline: true,
            multiline: false,
            scale: Matrix::identity(),
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn pos(mut self, x: f32, y: f32) -> Self {
        self.pos = (x, y);
        self
    }

    pub fn anchor(mut self, x: f32, y: f32) -> Self {
        self.anchor = (x, y);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
        self
    }

    pub fn no_baseline(mut self) -> Self {
        self.baseline = false;
        self
    }

    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    pub fn scale(mut self, scale: Matrix) -> Self {
        self.scale = scale;
        self
    }

    fn measure_inner<'c>(&mut self, text: &'c str) -> (Section<'c>, Rect) {
        let vp = get_viewport();
        let scale = 0.04 * self.size * vp.2 as f32;
        let mut section = Section::new().add_text(Text::new(text).with_scale(scale).with_color(self.color));
        let s = 2. / vp.2 as f32;
        if let Some(max_width) = self.max_width {
            section = section.with_bounds((max_width / s, f32::INFINITY));
        }
        if !self.multiline {
            section = section.with_layout(Layout::default_single_line());
        }
        let bound = self.ui.text_painter.brush.glyph_bounds(&section).unwrap_or_default();
        let mut height = bound.height();
        height += text.chars().take_while(|it| *it == '\n').count() as f32 * self.ui.text_painter.line_gap(scale) * 3.;
        if self.baseline {
            height += self.ui.text_painter.brush.fonts()[0].as_scaled(scale).descent();
        }
        let mut rect = Rect::new(self.pos.0, self.pos.1, bound.width() * s, height * s);
        rect.x -= rect.w * self.anchor.0;
        rect.y -= rect.h * self.anchor.1;
        (section, rect)
    }

    pub fn measure(&mut self) -> Rect {
        let text = self.text.take().unwrap();
        let (_, rect) = self.measure_inner(&text);
        self.text = Some(text);
        rect
    }

    pub fn draw(mut self) -> Rect {
        let text = std::mem::take(&mut self.text).unwrap();
        let (section, rect) = self.measure_inner(&text);
        let vp = get_viewport();
        let s = vp.2 as f32 / 2.;
        self.ui.text_painter.brush.queue(section.with_screen_position((rect.x * s, rect.y * s)));
        self.ui.with(Matrix::new_scaling(1. / s), |ui| {
            ui.apply(|ui| {
                ui.text_painter.submit();
            });
        });
        rect
    }
}

pub struct TextPainter {
    brush: GlyphBrush<[Vertex; 4]>,
    cache_texture: Texture2D,
    data_buffer: Vec<u8>,
    vertices_buffer: Vec<Vertex>,
}

impl TextPainter {
    pub fn new(font: FontArc) -> Self {
        let mut brush = GlyphBrushBuilder::using_font(font).build();
        brush.resize_texture(1024, 1024);
        // TODO optimize
        let cache_texture = Self::new_cache_texture(brush.texture_dimensions());
        Self {
            brush,
            cache_texture,
            data_buffer: Vec::new(),
            vertices_buffer: Vec::new(),
        }
    }

    fn new_cache_texture(dim: (u32, u32)) -> Texture2D {
        Texture2D::from_miniquad_texture(Texture::new_render_texture(
            unsafe { get_internal_gl() }.quad_context,
            TextureParams {
                width: dim.0,
                height: dim.1,
                filter: FilterMode::Linear,
                format: miniquad::TextureFormat::RGBA8,
                wrap: miniquad::TextureWrap::Clamp,
            },
        ))
    }

    pub fn line_gap(&self, scale: f32) -> f32 {
        self.brush.fonts()[0].as_scaled(scale).line_gap()
    }

    fn submit(&mut self) {
        let mut flushed = false;
        loop {
            match self.brush.process_queued(
                |rect, tex_data| unsafe {
                    if !flushed {
                        get_internal_gl().flush();
                        flushed = true;
                    }
                    use miniquad::gl::*;
                    glBindTexture(GL_TEXTURE_2D, self.cache_texture.raw_miniquad_texture_handle().gl_internal_id());
                    self.data_buffer.clear();
                    self.data_buffer.reserve(tex_data.len() * 4);
                    for alpha in tex_data {
                        self.data_buffer.extend_from_slice(&[255, 255, 255, *alpha]);
                    }
                    glTexSubImage2D(
                        GL_TEXTURE_2D,
                        0,
                        rect.min[0] as _,
                        rect.min[1] as _,
                        rect.width() as _,
                        rect.height() as _,
                        GL_RGBA,
                        GL_UNSIGNED_BYTE,
                        self.data_buffer.as_ptr() as _,
                    );
                },
                |vertex| {
                    let pos = &vertex.pixel_coords;
                    let uv = &vertex.tex_coords;
                    let color = vertex.extra.color.into();
                    [
                        Vertex::new(pos.min.x, pos.min.y, 0., uv.min.x, uv.min.y, color),
                        Vertex::new(pos.max.x, pos.min.y, 0., uv.max.x, uv.min.y, color),
                        Vertex::new(pos.min.x, pos.max.y, 0., uv.min.x, uv.max.y, color),
                        Vertex::new(pos.max.x, pos.max.y, 0., uv.max.x, uv.max.y, color),
                    ]
                },
            ) {
                Err(BrushError::TextureTooSmall { suggested }) => {
                    if !flushed {
                        unsafe { get_internal_gl() }.flush();
                        flushed = true;
                    }
                    self.cache_texture.delete();
                    self.cache_texture = Self::new_cache_texture(suggested);
                    self.brush.resize_texture(suggested.0, suggested.1);
                }
                Ok(BrushAction::Draw(vertices)) => {
                    self.vertices_buffer.clear();
                    self.vertices_buffer.extend(vertices.into_iter().flatten());
                    self.redraw();
                    break;
                }
                Ok(BrushAction::ReDraw) => {
                    self.redraw();
                    break;
                }
            }
        }
    }

    fn redraw(&self) {
        let gl = unsafe { get_internal_gl() }.quad_gl;
        gl.texture(Some(self.cache_texture));
        for vertices in self.vertices_buffer.chunks_exact(4) {
            gl.geometry(vertices, &[0, 2, 3, 0, 1, 3]);
        }
    }
}

impl Drop for TextPainter {
    fn drop(&mut self) {
        self.cache_texture.delete();
    }
}
