mod billboard;
pub use billboard::{BillBoard, Message, MessageHandle, MessageKind};

mod chart_info;
pub use chart_info::*;

mod dialog;
pub use dialog::Dialog;

mod scroll;
pub use scroll::Scroll;

mod shading;
pub use shading::*;

mod shadow;
pub use shadow::*;

mod text;
pub use text::{DrawText, TextPainter};

pub use glyph_brush::ab_glyph::FontArc;

use crate::{
    core::{Matrix, Point, Vector},
    ext::{get_viewport, nalgebra_to_glm, screen_aspect, source_of_image, RectExt, ScaleType, SafeTexture},
    judge::Judge,
    scene::{request_input, return_input, take_input},
};
use lyon::{
    lyon_tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor, StrokeOptions, StrokeTessellator, StrokeVertex,
        StrokeVertexConstructor, VertexBuffers,
    },
    math as lm,
    path::{Path, PathEvent},
};
use macroquad::prelude::*;
use miniquad::PassAction;
use std::{borrow::Cow, cell::RefCell, collections::HashMap, ops::Range};

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

struct ShadedConstructor<T: Shading>(Matrix, pub T);
impl<T: Shading> FillVertexConstructor<Vertex> for ShadedConstructor<T> {
    fn new_vertex(&mut self, vertex: FillVertex) -> Vertex {
        let pos = vertex.position();
        self.1.new_vertex(&self.0, &Point::new(pos.x, pos.y))
    }
}
impl<T: Shading> StrokeVertexConstructor<Vertex> for ShadedConstructor<T> {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> Vertex {
        let pos = vertex.position();
        self.1.new_vertex(&self.0, &Point::new(pos.x, pos.y))
    }
}

pub struct VertexBuilder<T: Shading> {
    matrix: Matrix,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    shading: T,
}

impl<T: Shading> VertexBuilder<T> {
    fn new(matrix: Matrix, shading: T) -> Self {
        Self {
            matrix,
            vertices: Vec::new(),
            indices: Vec::new(),
            shading,
        }
    }

    pub fn add(&mut self, x: f32, y: f32) {
        self.vertices.push(self.shading.new_vertex(&self.matrix, &Point::new(x, y)))
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

pub struct DRectButton {
    inner: RectButton,
    last_touching: bool,
    start_time: Option<f32>,
    config: ShadowConfig,
    delta: f32,
}
impl DRectButton {
    pub const TIME: f32 = 0.2;

    pub fn new() -> Self {
        Self {
            inner: RectButton::new(),
            last_touching: false,
            start_time: None,
            config: ShadowConfig::default(),
            delta: -0.006,
        }
    }

    fn build(&mut self, ui: &mut Ui, t: f32, r: Rect) -> (Rect, Path) {
        let r = r.feather((1. - self.progress(t)) * self.delta);
        self.inner.set(ui, r);
        (r, r.rounded(self.config.radius))
    }

    pub fn invalidate(&mut self) {
        self.inner.rect = Rect::default();
    }

    pub fn render_shadow<T: IntoShading>(&mut self, ui: &mut Ui, r: Rect, t: f32, alpha: f32, shading: impl FnOnce(Rect) -> T) -> (Rect, Path) {
        let (r, path) = self.build(ui, t, r);
        rounded_rect_shadow(
            ui,
            r,
            &ShadowConfig {
                elevation: self.config.elevation * self.progress(t),
                base: self.config.base * alpha,
                ..self.config
            },
        );
        ui.fill_path(&path, shading(r).into_shading());
        (r, path)
    }

    pub fn render_text<'a>(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        t: f32,
        alpha: f32,
        text: impl Into<Cow<'a, str>>,
        size: f32,
        chosen: bool,
    ) -> (Rect, Path) {
        let oh = r.h;
        let (r, path) = self.build(ui, t, r);
        let ct = r.center();
        ui.fill_path(
            &path,
            if chosen {
                Color::new(1., 1., 1., alpha)
            } else {
                Color::new(0., 0., 0., 0.4 * alpha)
            },
        );
        ui.text(text)
            .pos(ct.x, ct.y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(size * r.h / oh)
            .color(if chosen {
                Color::new(0.3, 0.3, 0.3, 1.)
            } else {
                Color::new(1., 1., 1., alpha)
            })
            .draw();
        (r, path)
    }

    #[inline]
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.config.radius = radius;
        self
    }

    #[inline]
    pub fn with_elevation(mut self, elevation: f32) -> Self {
        self.config.elevation = elevation;
        self
    }

    #[inline]
    pub fn with_base(mut self, base: f32) -> Self {
        self.config.base = base;
        self
    }

    #[inline]
    pub fn with_delta(mut self, delta: f32) -> Self {
        self.delta = delta;
        self
    }

    pub fn progress(&mut self, t: f32) -> f32 {
        if self.start_time.as_ref().map_or(false, |it| t > *it + Self::TIME) {
            self.start_time = None;
        }
        let p = if let Some(time) = &self.start_time {
            (t - time) / Self::TIME
        } else {
            1.
        };
        if self.inner.touching() {
            1. - p
        } else {
            p
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        let res = self.inner.touch(touch);
        let touching = self.inner.touching();
        if self.last_touching != touching {
            self.last_touching = touching;
            self.start_time = Some(t);
        }
        res
    }
}

thread_local! {
    static STATE: RefCell<HashMap<String, Option<u64>>> = RefCell::new(HashMap::new());
}

pub struct InputParams {
    password: bool,
    length: f32,
}

impl From<()> for InputParams {
    fn from(_: ()) -> Self {
        Self {
            password: false,
            length: 0.3,
        }
    }
}

impl From<bool> for InputParams {
    fn from(password: bool) -> Self {
        Self { password, ..().into() }
    }
}

impl From<f32> for InputParams {
    fn from(length: f32) -> Self {
        Self { length, ..().into() }
    }
}

pub struct Ui<'a> {
    pub top: f32,

    text_painter: &'a mut TextPainter,

    model_stack: Vec<Matrix>,
    touches: Option<Vec<Touch>>,

    vertex_buffers: VertexBuffers<Vertex, u16>,
    fill_tess: FillTessellator,
    fill_options: FillOptions,
    stroke_tess: StrokeTessellator,
    stroke_options: StrokeOptions,
}

impl<'a> Ui<'a> {
    pub fn new(text_painter: &'a mut TextPainter) -> Self {
        unsafe { get_internal_gl() }.quad_context.begin_default_pass(PassAction::Clear {
            depth: None,
            stencil: Some(0),
            color: None,
        });
        Self {
            top: 1. / screen_aspect(),

            text_painter,

            model_stack: vec![Matrix::identity()],
            touches: None,

            vertex_buffers: VertexBuffers::new(),
            fill_tess: FillTessellator::new(),
            fill_options: FillOptions::default(),
            stroke_tess: StrokeTessellator::new(),
            stroke_options: StrokeOptions::default(),
        }
    }

    fn ensure_touches(&mut self) -> &mut Vec<Touch> {
        if self.touches.is_none() {
            self.touches = Some(Judge::get_touches());
        }
        self.touches.as_mut().unwrap()
    }

    pub(crate) fn set_touches(&mut self, touches: Vec<Touch>) {
        self.touches = Some(touches);
    }

    pub fn builder<T: IntoShading>(&self, shading: T) -> VertexBuilder<T::Target> {
        VertexBuilder::new(self.get_matrix(), shading.into_shading())
    }

    pub fn fill_rect(&mut self, rect: Rect, shading: impl IntoShading) {
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
        let tol = 0.15 / (self.model_stack.last().unwrap().transform_vector(&Vector::new(1., 0.)).norm() * screen_width() / 2.);
        self.fill_options.tolerance = tol;
        self.stroke_options.tolerance = tol;
    }

    pub fn fill_path(&mut self, path: impl IntoIterator<Item = PathEvent>, shading: impl IntoShading) {
        self.set_tolerance();
        let shaded = ShadedConstructor(self.get_matrix(), shading.into_shading());
        let tex = shaded.1.texture();
        self.fill_tess
            .tessellate(path, &self.fill_options, &mut BuffersBuilder::new(&mut self.vertex_buffers, shaded))
            .unwrap();
        self.emit_lyon(tex);
    }

    pub fn fill_circle(&mut self, x: f32, y: f32, radius: f32, shading: impl IntoShading) {
        self.set_tolerance();
        let shaded = ShadedConstructor(self.get_matrix(), shading.into_shading());
        let tex = shaded.1.texture();
        self.fill_tess
            .tessellate_circle(lm::point(x, y), radius, &self.fill_options, &mut BuffersBuilder::new(&mut self.vertex_buffers, shaded))
            .unwrap();
        self.emit_lyon(tex);
    }

    pub fn stroke_circle(&mut self, x: f32, y: f32, radius: f32, width: f32, shading: impl IntoShading) {
        self.set_tolerance();
        let shaded = ShadedConstructor(self.get_matrix(), shading.into_shading());
        let tex = shaded.1.texture();
        self.stroke_options.line_width = width;
        self.stroke_tess
            .tessellate_circle(lm::point(x, y), radius, &self.stroke_options, &mut BuffersBuilder::new(&mut self.vertex_buffers, shaded))
            .unwrap();
        self.emit_lyon(tex);
    }

    pub fn stroke_path(&mut self, path: &Path, width: f32, shading: impl IntoShading) {
        self.set_tolerance();
        let shaded = ShadedConstructor(self.get_matrix(), shading.into_shading());
        let tex = shaded.1.texture();
        self.stroke_options.line_width = width;
        self.stroke_tess
            .tessellate_path(path, &self.stroke_options, &mut BuffersBuilder::new(&mut self.vertex_buffers, shaded))
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

    pub fn screen_rect(&self) -> Rect {
        Rect::new(-1., -self.top, 2., self.top * 2.)
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
    pub fn apply<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> R {
        unsafe { get_internal_gl() }
            .quad_gl
            .push_model_matrix(nalgebra_to_glm(self.model_stack.last().unwrap()));
        let res = f(self);
        unsafe { get_internal_gl() }.quad_gl.pop_model_matrix();
        res
    }

    pub fn scissor(&mut self, rect: Option<Rect>) {
        let igl = unsafe { get_internal_gl() };
        let gl = igl.quad_gl;
        if let Some(rect) = rect {
            let rect = self.rect_to_global(rect);
            let vp = get_viewport();
            let pt = (vp.0 as f32 + (rect.x + 1.) / 2. * vp.2 as f32, vp.1 as f32 + (rect.y * vp.2 as f32 / vp.3 as f32 + 1.) / 2. * vp.3 as f32);
            gl.scissor(Some((pt.0 as _, pt.1 as _, (rect.w * vp.2 as f32 / 2.) as _, (rect.h * vp.2 as f32 / 2.) as _)));
        } else {
            gl.scissor(None);
        }
    }

    pub fn text<'s, 'ui>(&'ui mut self, text: impl Into<Cow<'s, str>>) -> DrawText<'a, 's, 'ui> {
        DrawText::new(self, text.into())
    }

    fn clicked(&mut self, rect: Rect, entry: &mut Option<u64>) -> bool {
        let rect = self.rect_to_global(rect);
        let mut exists = false;
        let mut any = false;
        let old_entry = *entry;
        let mut res = false;
        self.ensure_touches().retain(|touch| {
            exists = exists || old_entry == Some(touch.id);
            if !rect.contains(touch.position) {
                return true;
            }
            any = true;
            match touch.phase {
                TouchPhase::Started => {
                    *entry = Some(touch.id);
                    false
                }
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if *entry != Some(touch.id) {
                        *entry = None;
                        true
                    } else {
                        false
                    }
                }
                TouchPhase::Cancelled => {
                    *entry = None;
                    true
                }
                TouchPhase::Ended => {
                    if entry.take() == Some(touch.id) {
                        res = true;
                        false
                    } else {
                        true
                    }
                }
            }
        });
        if res {
            return true;
        }
        if !any && exists {
            *entry = None;
        }
        false
    }

    pub fn accent(&self) -> Color {
        Color::from_rgba(0x21, 0x96, 0xf3, 0xff)
    }

    pub fn button(&mut self, id: &str, rect: Rect, text: impl Into<String>) -> bool {
        let text = text.into();
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let entry = state.entry(id.to_owned()).or_default();
            self.fill_rect(rect, if entry.is_some() { Color::new(1., 1., 1., 0.5) } else { WHITE });
            let ct = rect.center();
            self.text(text)
                .pos(ct.x, ct.y)
                .anchor(0.5, 0.5)
                .max_width(rect.w)
                .size(0.42)
                .color(BLACK)
                .no_baseline()
                .draw();
            self.clicked(rect, entry)
        })
    }

    pub fn checkbox(&mut self, text: impl Into<String>, value: &mut bool) -> Rect {
        let text = text.into();
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let entry = state.entry(format!("chkbox#{text}")).or_default();
            let w = 0.08;
            let s = 0.03;
            let text = self.text(text).pos(w, 0.).size(0.5).no_baseline().draw();
            let r = Rect::new(w / 2. - s, text.center().y - s, s * 2., s * 2.);
            self.fill_rect(r, if *value { self.accent() } else { WHITE });
            let r = Rect::new(r.x, r.y, text.right() - r.x, (text.bottom() - r.y).max(w));
            if self.clicked(r, entry) {
                *value ^= true;
            }
            r
        })
    }

    pub fn input(&mut self, label: impl Into<String>, value: &mut String, params: impl Into<InputParams>) -> Rect {
        let label = label.into();
        let params = params.into();
        let id = format!("input#{label}");
        let r = self.text(label).anchor(1., 0.).size(0.4).draw();
        let lf = r.x;
        let r = Rect::new(0.02, r.y - 0.01, params.length, r.h + 0.02);
        if if params.password {
            self.button(&id, r, &"*".repeat(value.chars().count()))
        } else {
            self.button(&id, r, value.as_str())
        } {
            request_input(&id, value);
        }
        if let Some((its_id, text)) = take_input() {
            if its_id == id {
                *value = text;
            } else {
                return_input(its_id, text);
            }
        }
        Rect::new(lf, r.y, r.right() - lf, r.h)
    }

    pub fn slider(&mut self, text: impl Into<String>, range: Range<f32>, step: f32, value: &mut f32, length: Option<f32>) -> Rect {
        let text = text.into();
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let entry = state.entry(text.clone()).or_default();

            let len = length.unwrap_or(0.3);
            let s = 0.002;
            let tr = self.text(format!("{text}: {value:.3}")).size(0.4).draw();
            let cy = tr.h + 0.03;
            let r = Rect::new(0., cy - s, len, s * 2.);
            self.fill_rect(r, WHITE);
            let p = (*value - range.start) / (range.end - range.start);
            let p = p.clamp(0., 1.);
            self.fill_circle(len * p, cy, 0.015, self.accent());
            let r = r.feather(0.015 - s);
            let r = self.rect_to_global(r);
            self.ensure_touches();
            if let Some(id) = entry {
                if let Some(touch) = self.touches.as_ref().unwrap().iter().rfind(|it| it.id == *id) {
                    let Vec2 { x, y } = touch.position;
                    let (x, _) = self.to_local((x, y));
                    let p = (x / len).clamp(0., 1.);
                    *value = range.start + (range.end - range.start) * p;
                    *value = (*value / step).round() * step;
                    if matches!(touch.phase, TouchPhase::Cancelled | TouchPhase::Ended) {
                        *entry = None;
                    }
                }
            } else if let Some(touch) = self.touches.as_ref().unwrap().iter().find(|it| r.contains(it.position)) {
                if touch.phase == TouchPhase::Started {
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
        })
    }

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

    pub fn avatar(&mut self, cx: f32, cy: f32, r: f32, c: Color, t: f32, avatar: Option<SafeTexture>) {
        rounded_rect_shadow(
            self,
            Rect::new(cx - r, cy - r, r * 2., r * 2.),
            &ShadowConfig {
                radius: r,
                ..Default::default()
            },
        );
        if let Some(avatar) = avatar {
            self.fill_circle(cx, cy, r, (*avatar, Rect::new(cx - r, cy - r, r * 2., r * 2.)));
        } else {
            self.loading(
                cx,
                cy,
                t,
                c,
                LoadingParams {
                    radius: r,
                    ..Default::default()
                },
            );
        }
        self.stroke_circle(cx, cy, r, 0.004, WHITE);
    }

    pub fn loading_path(start: f32, len: f32, r: f32) -> Path {
        use lyon::math::{point, vector, Angle};
        let mut path = Path::svg_builder();
        let pt = |a: f32| {
            let (sin, cos) = a.sin_cos();
            point(sin * r, cos * r)
        };
        path.move_to(pt(-start));
        path.arc(point(0., 0.), vector(r, r), Angle::radians(len), Angle::radians(0.));
        path.build()
    }

    const LOADING_SCALE: f32 = 0.74;
    const LOADING_CHANGE_SPEED: f32 = 3.5;
    const LOADING_ROTATE_SPEED: f32 = 4.1;

    pub fn loading(&mut self, cx: f32, cy: f32, t: f32, shading: impl IntoShading, params: impl Into<LoadingParams>) {
        use std::f32::consts::PI;

        let params = params.into();
        let (st, len) = if let Some(p) = params.progress {
            (t * Self::LOADING_ROTATE_SPEED, p * PI * 2.)
        } else {
            let ct = t * Self::LOADING_CHANGE_SPEED;
            let round = (ct / (PI * 2.)).floor();
            let st = round * Self::LOADING_SCALE + {
                let t = ct - round * PI * 2.;
                if t < PI {
                    0.
                } else {
                    ((t - PI * 3. / 2.).sin() + 1.) * Self::LOADING_SCALE / 2.
                }
            };
            let st = st * PI * 2. + t * Self::LOADING_ROTATE_SPEED;
            let len = (-ct.cos() * Self::LOADING_SCALE / 2. + 0.5) * PI * 2.;
            (st, len)
        };
        self.scope(|ui| {
            ui.dx(cx);
            ui.dy(cy);
            ui.stroke_path(&Self::loading_path(st, len, params.radius), params.width, shading);
        });
    }

    #[inline]
    pub fn back_rect(&self) -> Rect {
        Rect::new(-0.97, -self.top + 0.04, 0.1, 0.1)
    }
}

pub struct LoadingParams {
    radius: f32,
    width: f32,
    progress: Option<f32>,
}
impl Default for LoadingParams {
    fn default() -> Self {
        Self {
            radius: 0.05,
            width: 0.012,
            progress: None,
        }
    }
}
impl From<()> for LoadingParams {
    fn from(_: ()) -> Self {
        Self::default()
    }
}
impl From<f32> for LoadingParams {
    fn from(progress: f32) -> Self {
        Self {
            progress: Some(progress),
            ..Self::default()
        }
    }
}
