use anyhow::Result;
use macroquad::prelude::*;
use miniquad::{
    BlendFactor, BlendState, BlendValue, CompareFunc, Equation, PrimitiveType, StencilFaceState,
    StencilOp, StencilState,
};
use ordered_float::{Float, NotNan};
use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::Poll,
};

pub trait NotNanExt: Sized {
    fn not_nan(self) -> NotNan<Self>;
}

impl<T: Sized + Float> NotNanExt for T {
    fn not_nan(self) -> NotNan<Self> {
        NotNan::new(self).unwrap()
    }
}

pub fn draw_text_aligned(
    font: Font,
    text: &str,
    x: f32,
    y: f32,
    anchor: (f32, f32),
    scale: f32,
    color: Color,
) -> Rect {
    use macroquad::prelude::*;
    let size = (screen_width() / 23. * scale) as u16;
    let scale = 0.08 * scale / size as f32;
    let dim = measure_text(text, Some(font), size, scale);
    let rect = Rect::new(
        x - dim.width * anchor.0,
        y - dim.offset_y * anchor.1,
        dim.width,
        dim.offset_y,
    );
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

pub const PARALLELOGRAM_SLOPE: f32 = 0.13 / (7. / 13.);

pub fn draw_parallelogram(rect: Rect, texture: Option<(Texture2D, Rect)>, color: Color) {
    draw_parallelogram_ex(rect, texture, color, color);
}

pub fn draw_parallelogram_ex(rect: Rect, texture: Option<(Texture2D, Rect)>, top: Color, bottom: Color) {
    let l = rect.h * PARALLELOGRAM_SLOPE;
    let gl = unsafe { get_internal_gl() }.quad_gl;
    let p = if let Some((tex, tex_rect)) = texture {
        let lt = tex_rect.h * PARALLELOGRAM_SLOPE;
        gl.texture(Some(tex));
        [
            Vertex::new(rect.x + l, rect.y, 0., tex_rect.x + lt, tex_rect.y, top),
            Vertex::new(
                rect.right(),
                rect.y,
                0.,
                tex_rect.right(),
                tex_rect.y,
                top,
            ),
            Vertex::new(
                rect.x,
                rect.bottom(),
                0.,
                tex_rect.x,
                tex_rect.bottom(),
                bottom,
            ),
            Vertex::new(
                rect.right() - l,
                rect.bottom(),
                0.,
                tex_rect.right() - lt,
                tex_rect.bottom(),
                bottom,
            ),
        ]
    } else {
        gl.texture(None);
        [
            Vertex::new(rect.x + l, rect.y, 0., 0., 0., top),
            Vertex::new(rect.right(), rect.y, 0., 0., 0., top),
            Vertex::new(rect.x, rect.bottom(), 0., 0., 0., bottom),
            Vertex::new(rect.right() - l, rect.bottom(), 0., 0., 0., bottom),
        ]
    };
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&p, &[0, 2, 3, 0, 1, 3]);
}

pub fn thread_as_future<R: Send + 'static>(
    f: impl FnOnce() -> R + Send + 'static,
) -> impl Future<Output = R> {
    struct DummyFuture<R>(Arc<Mutex<Option<R>>>);
    impl<R> Future for DummyFuture<R> {
        type Output = R;

        fn poll(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> Poll<Self::Output> {
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

pub fn make_pipeline(
    write_color: bool,
    pass_op: StencilOp,
    test_func: CompareFunc,
    test_ref: i32,
) -> Result<GlPipeline> {
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
                Some(StencilState {
                    front: state,
                    back: state,
                })
            },
            primitive_type: PrimitiveType::Triangles,
            ..Default::default()
        },
        Vec::new(),
        Vec::new(),
    )
    .map_err(anyhow::Error::msg)
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
