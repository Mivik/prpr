mod scroll;
pub use scroll::Scroll;

use crate::{
    core::{Matrix, Point, Vector},
    ext::{draw_text_aligned, make_pipeline, screen_aspect, source_of_image, RectExt, ScaleType},
    judge::Judge,
};
use lyon::{
    lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, VertexBuffers},
    math as lm,
    path::PathEvent,
};
use macroquad::prelude::*;
use miniquad::{CompareFunc, PassAction, StencilOp};
use once_cell::sync::{Lazy, OnceCell};
use std::{cell::RefCell, collections::HashMap, ops::Range, sync::Mutex};

pub static FONT: OnceCell<Font> = OnceCell::new();

static PIPELINE_INC_STENCIL: Lazy<GlPipeline> = Lazy::new(|| make_pipeline(false, StencilOp::IncrementClamp, CompareFunc::Always, 0));
static PIPELINE_DEC_STENCIL: Lazy<GlPipeline> = Lazy::new(|| make_pipeline(false, StencilOp::DecrementClamp, CompareFunc::Always, 0));
thread_local! {
    static PIPELINE_CACHES: RefCell<Vec<GlPipeline>> = RefCell::default();
}

fn get_draw_pipeline(clips: u32) -> GlPipeline {
    PIPELINE_CACHES.with(|caches| {
        let mut caches = caches.borrow_mut();
        if let Some(cache) = caches.get(clips as usize).copied() {
            cache
        } else {
            for layer in (caches.len() as u32)..=clips {
                caches.push(make_pipeline(true, StencilOp::Keep, CompareFunc::Equal, layer as _));
            }
            *caches.last().unwrap()
        }
    })
}

#[derive(Default, Clone, Copy)]
pub struct Gravity(u8);

impl Gravity {
    pub const LEFT: u8 = 0;
    pub const HCENTER: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const TOP: u8 = 0;
    pub const VCENTER: u8 = 4;
    pub const BOTTOM: u8 = 8;

    pub const BEGIN: u8 = Self::LEFT | Self::TOP;
    pub const CENTER: u8 = Self::HCENTER | Self::VCENTER;
    pub const END: u8 = Self::RIGHT | Self::BOTTOM;

    fn value(mode: u8) -> f32 {
        match mode {
            0 => 0.,
            1 => 0.5,
            2 => 1.,
            _ => unreachable!(),
        }
    }

    pub fn offset(&self, total: (f32, f32), content: (f32, f32)) -> (f32, f32) {
        (Self::value(self.0 & 3) * (total.0 - content.0), Self::value((self.0 >> 2) & 3) * (total.1 - content.1))
    }

    pub fn from_point(&self, point: (f32, f32), content: (f32, f32)) -> (f32, f32) {
        (point.0 - content.0 * Self::value(self.0 & 3), point.1 - content.1 * Self::value((self.0 >> 2) & 3))
    }
}

impl From<u8> for Gravity {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

struct ShadedConstructor(Matrix, pub Shading);

impl FillVertexConstructor<Vertex> for ShadedConstructor {
    fn new_vertex(&mut self, vertex: FillVertex) -> Vertex {
        let pos = vertex.position();
        self.1.new_vertex(&self.0, pos.x, pos.y)
    }
}

pub struct DrawText<'a> {
    ui: &'a mut Ui,
    text: String,
    font: Option<Font>,
    size: f32,
    pos: (f32, f32),
    anchor: (f32, f32),
    color: Color,
    max_width: Option<f32>,
    multiline: bool,
}

impl<'a> DrawText<'a> {
    fn new(ui: &'a mut Ui, text: String) -> Self {
        Self {
            ui,
            text,
            font: None,
            size: 1.,
            pos: (0., 0.),
            anchor: (0., 0.),
            color: WHITE,
            max_width: None,
            multiline: false,
        }
    }

    #[must_use]
    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn pos(mut self, x: f32, y: f32) -> Self {
        self.pos = (x, y);
        self
    }

    #[must_use]
    pub fn anchor(mut self, x: f32, y: f32) -> Self {
        self.anchor = (x, y);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
        self
    }

    #[must_use]
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    #[must_use]
    pub fn measure(&self) -> Rect {
        let size = (screen_width() / 23. * self.size) as u16;
        let scale = 0.08 * self.size / size as f32;
        let dim = measure_text(&self.text, Some(self.font.unwrap_or_else(|| *FONT.get().unwrap())), size, scale);
        Rect::new(self.pos.0 - dim.width * self.anchor.0, self.pos.1 - dim.offset_y * self.anchor.1, dim.width, dim.offset_y)
    }

    pub fn draw(mut self) -> Rect {
        let mut tmp = None;
        if let Some(width) = self.max_width {
            if self.measure().w > width || self.multiline {
                let text = std::mem::take(&mut self.text);
                for ch in text.chars() {
                    self.text.push(ch);
                    if self.measure().w > width || (self.multiline && ch == '\n') {
                        if ch != '\n' {
                            self.text.pop();
                        }
                        break;
                    }
                }
                tmp = Some(text);
            } else {
                tmp = Some(self.text.clone());
            }
        }
        let mut res = self.ui.apply(|| {
            draw_text_aligned(
                self.font.unwrap_or_else(|| *FONT.get().unwrap()),
                &self.text,
                self.pos.0,
                self.pos.1,
                self.anchor,
                self.size,
                self.color,
            )
        });
        if self.multiline {
            // only supports anchor (0, 0)
            if self.text.len() < tmp.as_ref().unwrap().len() {
                self.text = tmp.unwrap()[self.text.len()..].to_string();
                res.h += 0.01;
                self.pos.1 += res.h;
                let new = self.draw();
                res.w = res.w.max(res.w);
                res.h += new.h;
            }
        }
        res
    }
}

pub struct Shading {
    color: Color,
    texture: Option<(Texture2D, Rect, Rect)>,
}

impl Shading {
    pub fn new_vertex(&self, matrix: &Matrix, x: f32, y: f32) -> Vertex {
        let p = matrix.transform_point(&Point::new(x, y));
        if let Some((_, tr, dr)) = self.texture {
            let ux = (x - dr.x) / dr.w;
            let uy = (y - dr.y) / dr.h;
            let ux = ux.max(0.).min(1.);
            let uy = uy.max(0.).min(1.);
            Vertex::new(p.x, p.y, 0., tr.x + tr.w * ux, tr.y + tr.h * uy, self.color)
        } else {
            Vertex::new(p.x, p.y, 0., 0., 0., self.color)
        }
    }

    pub fn texture(&self) -> Option<Texture2D> {
        self.texture.map(|it| it.0)
    }
}

impl From<Color> for Shading {
    fn from(color: Color) -> Self {
        Self { color, texture: None }
    }
}

impl From<(Texture2D, Rect)> for Shading {
    fn from((tex, rect): (Texture2D, Rect)) -> Self {
        (tex, rect, ScaleType::default(), WHITE).into()
    }
}

impl From<(Texture2D, Rect, ScaleType)> for Shading {
    fn from((tex, rect, scale_type): (Texture2D, Rect, ScaleType)) -> Self {
        (tex, rect, scale_type, WHITE).into()
    }
}

impl From<(Texture2D, Rect, ScaleType, Color)> for Shading {
    fn from((tex, rect, scale_type, color): (Texture2D, Rect, ScaleType, Color)) -> Self {
        let source = source_of_image(&tex, rect, scale_type).unwrap_or_else(|| Rect::new(0., 0., 1., 1.));
        Self {
            color,
            texture: Some((tex, source, rect)),
        }
    }
}

pub struct VertexBuilder {
    matrix: Matrix,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    shading: Shading,
}

impl VertexBuilder {
    fn new(matrix: Matrix, shading: Shading) -> Self {
        Self {
            matrix,
            vertices: Vec::new(),
            indices: Vec::new(),
            shading,
        }
    }

    pub fn add(&mut self, x: f32, y: f32) {
        self.vertices.push(self.shading.new_vertex(&self.matrix, x, y))
    }

    pub fn triangle(&mut self, x: u16, y: u16, z: u16) {
        self.indices.push(x);
        self.indices.push(y);
        self.indices.push(z);
    }

    pub fn commit(&self) {
        let gl = unsafe { get_internal_gl() }.quad_gl;
        gl.texture(self.shading.texture());
        gl.draw_mode(DrawMode::Triangles);
        gl.geometry(&self.vertices, &self.indices);
    }
}

#[derive(Clone, Copy)]
pub struct RectButton {
    pub rect: Rect,
    id: Option<u64>,
}

impl Default for RectButton {
    fn default() -> Self {
        Self::new()
    }
}

impl RectButton {
    pub fn new() -> Self {
        Self {
            rect: Rect::default(),
            id: None,
        }
    }

    pub fn touching(&self) -> bool {
        self.id.is_some()
    }

    pub fn set(&mut self, ui: &mut Ui, rect: Rect) {
        self.rect = ui.rect_to_global(rect);
    }

    pub fn touch(&mut self, touch: &Touch) -> bool {
        let inside = self.rect.contains(touch.position);
        match touch.phase {
            TouchPhase::Started => {
                if inside {
                    self.id = Some(touch.id);
                }
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                if self.id == Some(touch.id) && !inside {
                    self.id = None;
                }
            }
            TouchPhase::Cancelled => {
                self.id = None;
            }
            TouchPhase::Ended => {
                if self.id.take() == Some(touch.id) && inside {
                    return true;
                }
            }
        }
        false
    }
}

static STATE: Lazy<Mutex<HashMap<String, Option<u64>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub struct Ui {
    pub top: f32,

    clips: u32,
    model_stack: Vec<Matrix>,
    touches: Vec<Touch>,

    vertex_buffers: VertexBuffers<Vertex, u16>,
    fill_tess: FillTessellator,
    fill_options: FillOptions,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        unsafe { get_internal_gl() }.quad_context.begin_default_pass(PassAction::Clear {
            depth: None,
            stencil: Some(0),
            color: None,
        });
        Self {
            top: 1. / screen_aspect(),

            clips: 0,
            model_stack: vec![Matrix::identity()],
            touches: Judge::get_touches(),

            vertex_buffers: VertexBuffers::new(),
            fill_tess: FillTessellator::new(),
            fill_options: FillOptions::default(),
        }
    }

    pub fn builder(&self, shading: impl Into<Shading>) -> VertexBuilder {
        VertexBuilder::new(self.get_matrix(), shading.into())
    }

    pub fn fill_rect(&mut self, rect: Rect, shading: impl Into<Shading>) {
        let mut b = self.builder(shading);
        b.add(rect.x, rect.y);
        b.add(rect.x + rect.w, rect.y);
        b.add(rect.x, rect.y + rect.h);
        b.add(rect.x + rect.w, rect.y + rect.h);
        b.triangle(0, 1, 2);
        b.triangle(1, 2, 3);
        b.commit();
    }

    fn set_tolerance(&mut self) {
        let tol = 0.2 / (self.model_stack.last().unwrap().transform_vector(&Vector::new(1., 0.)).norm() * screen_width() / 2.);
        self.fill_options.tolerance = tol;
    }

    pub fn fill_path(&mut self, path: impl IntoIterator<Item = PathEvent>, shading: impl Into<Shading>) {
        self.set_tolerance();
        let shaded = ShadedConstructor(self.get_matrix(), shading.into());
        let tex = shaded.1.texture();
        self.fill_tess
            .tessellate(path, &self.fill_options, &mut BuffersBuilder::new(&mut self.vertex_buffers, shaded))
            .unwrap();
        self.emit_lyon(tex);
    }

    pub fn fill_circle(&mut self, x: f32, y: f32, radius: f32, shading: impl Into<Shading>) {
        self.set_tolerance();
        let shaded = ShadedConstructor(self.get_matrix(), shading.into());
        let tex = shaded.1.texture();
        self.fill_tess
            .tessellate_circle(lm::point(x, y), radius, &self.fill_options, &mut BuffersBuilder::new(&mut self.vertex_buffers, shaded))
            .unwrap();
        self.emit_lyon(tex);
    }

    fn emit_lyon(&mut self, texture: Option<Texture2D>) {
        let gl = unsafe { get_internal_gl() }.quad_gl;
        gl.texture(texture);
        gl.draw_mode(DrawMode::Triangles);
        gl.geometry(&std::mem::take(&mut self.vertex_buffers.vertices), &std::mem::take(&mut self.vertex_buffers.indices));
    }

    pub fn get_matrix(&self) -> Matrix {
        *self.model_stack.last().unwrap()
    }

    pub fn rect_to_global(&self, rect: Rect) -> Rect {
        let pt = self.to_global((rect.x, rect.y));
        let vec = self.vec_to_global((rect.w, rect.h));
        Rect::new(pt.0, pt.1, vec.0, vec.1)
    }

    pub fn vec_to_global(&self, vec: (f32, f32)) -> (f32, f32) {
        let r = self.model_stack.last().unwrap().transform_vector(&Vector::new(vec.0, vec.1));
        (r.x, r.y)
    }

    pub fn to_global(&self, pt: (f32, f32)) -> (f32, f32) {
        let r = self.model_stack.last().unwrap().transform_point(&Point::new(pt.0, pt.1));
        (r.x, r.y)
    }

    pub fn to_local(&self, pt: (f32, f32)) -> (f32, f32) {
        let r = self
            .model_stack
            .last()
            .unwrap()
            .try_inverse()
            .unwrap()
            .transform_point(&Point::new(pt.0, pt.1));
        (r.x, r.y)
    }

    pub fn dx(&mut self, x: f32) {
        self.model_stack.last_mut().unwrap().append_translation_mut(&Vector::new(x, 0.));
    }

    pub fn dy(&mut self, y: f32) {
        self.model_stack.last_mut().unwrap().append_translation_mut(&Vector::new(0., y));
    }

    #[inline]
    pub fn with<R>(&mut self, model: Matrix, f: impl FnOnce(&mut Self) -> R) -> R {
        let model = self.model_stack.last().unwrap() * model;
        self.model_stack.push(model);
        let res = f(self);
        self.model_stack.pop();
        res
    }

    #[inline]
    pub fn scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let model = *self.model_stack.last().unwrap();
        self.model_stack.push(model);
        let res = f(self);
        self.model_stack.pop();
        res
    }

    #[inline]
    pub fn apply<R>(&self, f: impl FnOnce() -> R) -> R {
        self.apply_model_of(self.model_stack.last().unwrap(), f)
    }

    #[inline]
    fn apply_model_of<R>(&self, mat: &Matrix, f: impl FnOnce() -> R) -> R {
        unsafe { get_internal_gl() }.quad_gl.push_model_matrix({
            /*
                [11] [12]  0  [13]
                [21] [22]  0  [23]
                  0    0   1    0
                [31] [32]  0  [33]
            */
            Mat4::from_cols_array(&[
                mat.m11, mat.m21, 0., mat.m31, mat.m12, mat.m22, 0., mat.m32, 0., 0., 1., 0., mat.m13, mat.m23, 0., mat.m33,
            ])
        });
        let res = f();
        unsafe { get_internal_gl() }.quad_gl.pop_model_matrix();
        res
    }

    pub fn clipped(&mut self, mut f: impl FnMut(&mut Ui), g: impl FnOnce(&mut Ui)) {
        let gl = unsafe { get_internal_gl() }.quad_gl;
        gl.pipeline(Some(*PIPELINE_INC_STENCIL));
        f(self);
        self.clips += 1;
        gl.pipeline(Some(get_draw_pipeline(self.clips)));
        g(self);
        gl.pipeline(Some(*PIPELINE_DEC_STENCIL));
        f(self);
        self.clips -= 1;
        gl.pipeline(if self.clips == 0 { None } else { Some(get_draw_pipeline(self.clips)) });
    }

    pub fn scissor(&mut self, rect: Option<Rect>) {
        let igl = unsafe { get_internal_gl() };
        let gl = igl.quad_gl;
        if let Some(rect) = rect {
            let rect = self.rect_to_global(rect);
            let vp = gl
                .get_active_render_pass()
                .map(|it| {
                    let tex = it.texture(igl.quad_context);
                    (0, 0, tex.width as i32, tex.height as i32)
                })
                .unwrap_or_else(|| gl.get_viewport());
            let pt = (vp.0 as f32 + (rect.x + 1.) / 2. * vp.2 as f32, vp.1 as f32 + (rect.y * vp.2 as f32 / vp.3 as f32 + 1.) / 2. * vp.3 as f32);
            gl.scissor(Some((pt.0 as _, pt.1 as _, (rect.w * vp.2 as f32 / 2.) as _, (rect.h * vp.2 as f32 / 2.) as _)));
        } else {
            gl.scissor(None);
        }
    }

    #[must_use]
    pub fn text(&mut self, text: impl Into<String>) -> DrawText<'_> {
        DrawText::new(self, text.into())
    }

    fn clicked(&self, rect: Rect, entry: &mut Option<u64>) -> bool {
        let rect = self.rect_to_global(rect);
        if let Some(touch) = self.touches.iter().find(|it| rect.contains(it.position)) {
            match touch.phase {
                TouchPhase::Started => {
                    *entry = Some(touch.id);
                }
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if *entry != Some(touch.id) {
                        *entry = None;
                    }
                }
                TouchPhase::Cancelled => {
                    *entry = None;
                }
                TouchPhase::Ended => {
                    if entry.take() == Some(touch.id) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn accent(&self) -> Color {
        Color::from_rgba(0x21, 0x96, 0xf3, 0xff)
    }

    pub fn checkbox(&mut self, text: impl Into<String>, value: &mut bool) -> Rect {
        let text = text.into();
        let mut state = STATE.lock().unwrap();
        let entry = state.entry(text.clone()).or_default();
        let w = 0.08;
        let s = 0.03;
        let text = self.text(text).pos(w, 0.).size(0.5).draw();
        let r = Rect::new(w / 2. - s, text.center().y - s, s * 2., s * 2.);
        self.fill_rect(r, if *value { self.accent() } else { WHITE });
        let r = Rect::new(r.x, r.y, text.right() - r.x, (text.bottom() - r.y).max(w));
        if self.clicked(r, entry) {
            *value ^= true;
        }
        r
    }

    pub fn slider(&mut self, text: impl Into<String>, range: Range<f32>, step: f32, value: &mut f32, length: Option<f32>) -> Rect {
        let text = text.into();
        let mut state = STATE.lock().unwrap();
        let entry = state.entry(text.clone()).or_default();

        let len = length.unwrap_or(0.3);
        let s = 0.002;
        let tr = self.text(format!("{text}: {value:.3}")).size(0.4).draw();
        let cy = tr.h + 0.03;
        let r = Rect::new(0., cy - s, len, s * 2.);
        self.fill_rect(r, WHITE);
        let p = (*value - range.start) / (range.end - range.start);
        let p = p.max(0.).min(1.);
        self.fill_circle(len * p, cy, 0.015, self.accent());
        let r = r.feather(0.015 - s);
        let r = self.rect_to_global(r);
        if let Some(id) = entry {
            if let Some(touch) = self.touches.iter().rfind(|it| it.id == *id) {
                let Vec2 { x, y } = touch.position;
                let (x, _) = self.to_local((x, y));
                let p = (x / len).max(0.).min(1.);
                *value = range.start + (range.end - range.start) * p;
                *value = (*value / step).round() * step;
                if matches!(touch.phase, TouchPhase::Cancelled | TouchPhase::Ended) {
                    *entry = None;
                }
            }
        } else if let Some(touch) = self.touches.iter().find(|it| r.contains(it.position)) {
            if matches!(touch.phase, TouchPhase::Started) {
                *entry = Some(touch.id);
            }
        }

        let s = 0.025;
        let mut x = len + 0.02;
        let r = Rect::new(x, cy - s, s * 2., s * 2.);
        self.fill_rect(r, WHITE);
        self.text("-")
            .pos(r.center().x, r.center().y)
            .anchor(0.5, 0.5)
            .size(0.4)
            .color(BLACK)
            .draw();
        if self.clicked(r, state.entry(format!("{text}:-")).or_default()) {
            *value = (*value - step).max(range.start);
        }
        x += s * 2. + 0.01;
        let r = Rect::new(x, cy - s, s * 2., s * 2.);
        self.fill_rect(r, WHITE);
        self.text("+")
            .pos(r.center().x, r.center().y)
            .anchor(0.5, 0.5)
            .size(0.4)
            .color(BLACK)
            .draw();
        if self.clicked(r, state.entry(format!("{text}:+")).or_default()) {
            *value = (*value + step).min(range.end);
        }

        Rect::new(0., 0., x + s * 2., cy + s)
    }

    #[inline]
    pub fn hgrids(&mut self, width: f32, height: f32, row_num: u32, count: u32, mut content: impl FnMut(&mut Self, u32)) -> (f32, f32) {
        let mut sh = 0.;
        let w = width / row_num as f32;
        for i in (0..count).step_by(row_num as usize) {
            let mut sw = 0.;
            for j in 0..(count - i).min(row_num) {
                content(self, i + j);
                self.dx(w);
                sw += w;
            }
            self.dx(-sw);
            self.dy(height);
            sh += height;
        }
        self.dy(-sh);
        (width, sh)
    }
}
