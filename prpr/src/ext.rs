use crate::core::{Point, Vector};
use image::DynamicImage;
use macroquad::prelude::*;
use miniquad::{BlendFactor, BlendState, BlendValue, CompareFunc, Equation, PrimitiveType, StencilFaceState, StencilOp, StencilState};
use once_cell::sync::Lazy;
use ordered_float::{Float, NotNan};
use std::{
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, RawWaker, RawWakerVTable, Waker},
};

pub trait JoinToString {
    fn join(self, sep: &str) -> String;
}

impl<V: AsRef<str>, T: Iterator<Item = V>> JoinToString for T {
    fn join(mut self, sep: &str) -> String {
        let mut result = String::new();
        if let Some(first) = self.next() {
            result += first.as_ref();
            for element in self {
                result += sep;
                result += element.as_ref();
            }
        }
        result
    }
}

pub trait NotNanExt: Sized {
    fn not_nan(self) -> NotNan<Self>;
}

impl<T: Sized + Float> NotNanExt for T {
    fn not_nan(self) -> NotNan<Self> {
        NotNan::new(self).unwrap()
    }
}

pub trait RectExt: Sized {
    fn feather(&self, radius: f32) -> Self;
}

impl RectExt for Rect {
    fn feather(&self, radius: f32) -> Self {
        Self::new(self.x - radius, self.y - radius, self.w + radius * 2., self.h + radius * 2.)
    }
}

struct SafeTextureWrapper(Texture2D);
impl Drop for SafeTextureWrapper {
    fn drop(&mut self) {
        self.0.delete()
    }
}

pub struct SafeTexture(Arc<SafeTextureWrapper>);
impl SafeTexture {
    pub fn into_inner(self) -> Texture2D {
        let arc = self.0;
        let res = arc.0;
        std::mem::forget(arc);
        res
    }
}

impl Clone for SafeTexture {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl Deref for SafeTexture {
    type Target = Texture2D;

    fn deref(&self) -> &Self::Target {
        &self.0.as_ref().0
    }
}

impl From<Texture2D> for SafeTexture {
    fn from(tex: Texture2D) -> Self {
        Self(Arc::new(SafeTextureWrapper(tex)))
    }
}

impl From<DynamicImage> for SafeTexture {
    fn from(image: DynamicImage) -> Self {
        Texture2D::from_rgba8(image.width() as _, image.height() as _, &image.into_rgba8()).into()
    }
}

pub static BLACK_TEXTURE: Lazy<SafeTexture> = Lazy::new(|| Texture2D::from_rgba8(1, 1, &[0, 0, 0, 255]).into());

pub fn get_viewport() -> (i32, i32, i32, i32) {
    let gl = unsafe { get_internal_gl() };
    gl.quad_gl
        .get_active_render_pass()
        .map(|it| {
            let tex = it.texture(gl.quad_context);
            (0, 0, tex.width as i32, tex.height as i32)
        })
        .unwrap_or_else(|| gl.quad_gl.get_viewport())
}

pub fn draw_text_aligned(font: Font, text: &str, x: f32, y: f32, anchor: (f32, f32), scale: f32, color: Color) -> Rect {
    use macroquad::prelude::*;
    let size = (get_viewport().2 as f32 / 23. * scale) as u16;
    let scale = 0.08 * scale / size as f32;
    let dim = measure_text(text, Some(font), size, scale);
    let rect = Rect::new(x - dim.width * anchor.0, y - dim.offset_y * anchor.1, dim.width, dim.offset_y);
    draw_text_ex(
        text,
        rect.x,
        rect.y + dim.offset_y,
        TextParams {
            font,
            font_size: size,
            font_scale: scale,
            color,
            ..Default::default()
        },
    );
    rect
}

#[derive(Default, Clone, Copy)]
pub enum ScaleType {
    #[default]
    Scale,
    Inside,
    Fit,
}

pub fn source_of_image(tex: &Texture2D, rect: Rect, scale_type: ScaleType) -> Option<Rect> {
    match scale_type {
        ScaleType::Scale => {
            let exp = rect.w / rect.h;
            let act = tex.width() / tex.height();
            Some(if exp > act {
                let h = act / exp;
                Rect::new(0., 0.5 - h / 2., 1., h)
            } else {
                let w = exp / act;
                Rect::new(0.5 - w / 2., 0., w, 1.)
            })
        }
        ScaleType::Inside => {
            let exp = rect.w / rect.h;
            let act = tex.width() / tex.height();
            Some(if exp > act {
                let w = act / exp;
                Rect::new(0.5 - w / 2., 0., w, 1.)
            } else {
                let h = exp / act;
                Rect::new(0., 0.5 - h / 2., 1., h)
            })
        }
        ScaleType::Fit => None,
    }
}

pub fn draw_image(tex: Texture2D, rect: Rect, scale_type: ScaleType) {
    let source = source_of_image(&tex, rect, scale_type);
    let (w, h) = (tex.width(), tex.height());
    draw_texture_ex(
        tex,
        rect.x,
        rect.y,
        WHITE,
        DrawTextureParams {
            source: source.map(|it| Rect::new(it.x * w, it.y * h, it.w * w, it.h * h)),
            dest_size: Some(rect.size()),
            ..Default::default()
        },
    );
}

pub const PARALLELOGRAM_SLOPE: f32 = 0.13 / (7. / 13.);

pub fn draw_parallelogram(rect: Rect, texture: Option<(Texture2D, Rect)>, color: Color, shadow: bool) {
    draw_parallelogram_ex(rect, texture, color, color, shadow);
}

pub fn draw_parallelogram_ex(rect: Rect, texture: Option<(Texture2D, Rect)>, top: Color, bottom: Color, shadow: bool) {
    let l = rect.h * PARALLELOGRAM_SLOPE;
    let gl = unsafe { get_internal_gl() }.quad_gl;
    let p = [
        Point::new(rect.x + l, rect.y),
        Point::new(rect.right(), rect.y),
        Point::new(rect.x, rect.bottom()),
        Point::new(rect.right() - l, rect.bottom()),
    ];
    let v = if let Some((tex, tex_rect)) = texture {
        let lt = tex_rect.h * tex.height() * PARALLELOGRAM_SLOPE / tex.width();
        gl.texture(Some(tex));
        [
            Vertex::new(p[0].x, p[0].y, 0., tex_rect.x + lt, tex_rect.y, top),
            Vertex::new(p[1].x, p[1].y, 0., tex_rect.right(), tex_rect.y, top),
            Vertex::new(p[2].x, p[2].y, 0., tex_rect.x, tex_rect.bottom(), bottom),
            Vertex::new(p[3].x, p[3].y, 0., tex_rect.right() - lt, tex_rect.bottom(), bottom),
        ]
    } else {
        gl.texture(None);
        [
            Vertex::new(p[0].x, p[0].y, 0., 0., 0., top),
            Vertex::new(p[1].x, p[1].y, 0., 0., 0., top),
            Vertex::new(p[2].x, p[2].y, 0., 0., 0., bottom),
            Vertex::new(p[3].x, p[3].y, 0., 0., 0., bottom),
        ]
    };
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&v, &[0, 2, 3, 0, 1, 3]);
    if shadow {
        drop_shadow(p, top.a.min(bottom.a));
    }
}

fn drop_shadow(p: [Point; 4], alpha: f32) {
    const RADIUS: f32 = 0.018;
    let len = (PARALLELOGRAM_SLOPE * PARALLELOGRAM_SLOPE + 1.).sqrt();
    let n1 = Vector::new(PARALLELOGRAM_SLOPE / len - 1., -1. / len) * RADIUS;
    let n2 = Vector::new(n1.x + RADIUS * 2., n1.y);
    let c1 = Color::new(0., 0., 0., alpha * 0.11);
    let c2 = Color::default();
    let v = |p: Point, c: Color| Vertex::new(p.x, p.y, 0., 0., 0., c);
    let p = [
        v(p[0], c1),
        v(p[0] + n1, c2),
        v(p[1], c1),
        v(p[1] + n2, c2),
        v(p[2], c1),
        v(p[2] - n2, c2),
        v(p[3], c1),
        v(p[3] - n1, c2),
    ];
    let gl = unsafe { get_internal_gl() }.quad_gl;
    gl.texture(None);
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&p, &[0, 1, 2, 1, 2, 3, 0, 1, 5, 0, 5, 4, 4, 5, 6, 5, 6, 7, 6, 7, 2, 7, 2, 3]);
}

pub fn thread_as_future<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> impl Future<Output = R> {
    struct DummyFuture<R>(Arc<Mutex<Option<R>>>);
    impl<R> Future for DummyFuture<R> {
        type Output = R;

        fn poll(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> Poll<Self::Output> {
            match self.0.lock().unwrap().take() {
                Some(res) => Poll::Ready(res),
                None => Poll::Pending,
            }
        }
    }
    let arc = Arc::new(Mutex::new(None));
    std::thread::spawn({
        let arc = Arc::clone(&arc);
        move || {
            let res = f();
            *arc.lock().unwrap() = Some(res);
        }
    });
    DummyFuture(arc)
}

pub fn spawn_task<R: Send + 'static>(future: impl Future<Output = R> + Send + 'static) -> impl Future<Output = anyhow::Result<R>> {
    #[cfg(target_arch = "wasm32")]
    {
        async move { Ok(future.await) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        async move { Ok(tokio::spawn(future).await?) }
    }
}

pub fn poll_future<R>(future: Pin<&mut (impl Future<Output = R> + ?Sized)>) -> Option<R> {
    fn waker() -> Waker {
        unsafe fn clone(data: *const ()) -> RawWaker {
            RawWaker::new(data, &VTABLE)
        }
        unsafe fn wake(_data: *const ()) {
            // panic!()
        }
        unsafe fn wake_by_ref(data: *const ()) {
            wake(data)
        }
        unsafe fn drop(_data: *const ()) {}
        const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
        unsafe { Waker::from_raw(raw_waker) }
    }
    let waker = waker();
    let mut futures_context = std::task::Context::from_waker(&waker);
    match future.poll(&mut futures_context) {
        Poll::Ready(val) => Some(val),
        Poll::Pending => None,
    }
}

pub fn screen_aspect() -> f32 {
    let vp = unsafe { get_internal_gl() }.quad_gl.get_viewport();
    vp.2 as f32 / vp.3 as f32
}

pub fn make_pipeline(write_color: bool, pass_op: StencilOp, test_func: CompareFunc, test_ref: i32) -> GlPipeline {
    let InternalGlContext {
        quad_gl: gl,
        quad_context: context,
    } = unsafe { get_internal_gl() };
    gl.make_pipeline(
        context,
        shader::VERTEX,
        shader::FRAGMENT,
        PipelineParams {
            color_write: (write_color, write_color, write_color, write_color),
            color_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::Value(BlendValue::SourceAlpha),
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            stencil_test: {
                let state = StencilFaceState {
                    fail_op: StencilOp::Keep,
                    depth_fail_op: StencilOp::Keep,
                    pass_op,
                    test_func,
                    test_ref,
                    test_mask: u32::MAX,
                    write_mask: u32::MAX,
                };
                Some(StencilState { front: state, back: state })
            },
            primitive_type: PrimitiveType::Triangles,
            ..Default::default()
        },
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
}

mod shader {
    pub const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

    pub const FRAGMENT: &str = r#"#version 100
varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    gl_FragColor = color * texture2D(Texture, uv) ;
}"#;
}
