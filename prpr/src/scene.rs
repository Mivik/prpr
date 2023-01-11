mod ending;
use cfg_if::cfg_if;
pub use ending::EndingScene;

mod game;
pub use game::GameScene;

mod loading;
pub use loading::LoadingScene;

use crate::{
    ext::{draw_image, screen_aspect, ScaleType},
    judge::Judge,
    time::TimeManager,
    ui::{BillBoard, Dialog, Ui},
};
use anyhow::{Error, Result};
use macroquad::prelude::*;
use std::{cell::RefCell, ops::DerefMut, sync::Mutex};

#[derive(Default)]
pub enum NextScene {
    #[default]
    None,
    Pop,
    PopN(usize),
    Exit,
    Overlay(Box<dyn Scene>),
    Replace(Box<dyn Scene>),
}

thread_local! {
    pub static BILLBOARD: RefCell<(BillBoard, TimeManager)> = RefCell::new((BillBoard::new(), TimeManager::default()));
    pub static DIALOG: RefCell<Option<Dialog>> = RefCell::new(None);
}

#[inline]
pub fn show_error(error: Error) {
    Dialog::error(error).show();
}

pub fn show_message(msg: impl Into<String>) {
    BILLBOARD.with(|it| {
        let mut guard = it.borrow_mut();
        let t = guard.1.now() as _;
        guard.0.add(msg, t);
    });
}

thread_local! {
    static CURRENT_INPUT: RefCell<String> = RefCell::default();
    #[cfg(not(target_arch = "wasm32"))]
    static CURRENT_CHOOSE_FILE: RefCell<String> = RefCell::default();
}
pub static INPUT_TEXT: Mutex<Option<String>> = Mutex::new(None);
#[cfg(not(target_arch = "wasm32"))]
pub static CHOSEN_FILE: Mutex<Option<String>> = Mutex::new(None);

pub fn request_input(id: impl Into<String>, #[allow(unused_variables)] text: &str) {
    CURRENT_INPUT.with(|it| *it.borrow_mut() = id.into());
    cfg_if! {
        if #[cfg(target_os = "android")] {
            unsafe {
                let env = miniquad::native::attach_jni_env();
                let ctx = ndk_context::android_context().context();
                let class = (**env).GetObjectClass.unwrap()(env, ctx);
                let method = (**env).GetMethodID.unwrap()(env, class, b"inputText\0".as_ptr() as _, b"(Ljava/lang/String;)V\0".as_ptr() as _);
                let text = std::ffi::CString::new(text.to_owned()).unwrap();
                (**env).CallVoidMethod.unwrap()(env, ctx, method, (**env).NewStringUTF.unwrap()(env, text.as_ptr()));
            }
        } else if #[cfg(target_os = "ios")] {
            unsafe {
                use crate::objc::*;
                let view_ctrl = *miniquad::native::ios::VIEW_CTRL_OBJ.lock().unwrap();

                let alert: ObjcId = msg_send![
                    class!(UIAlertController),
                    alertControllerWithTitle: str_to_ns("输入")
                    message: str_to_ns("请输入文字")
                    preferredStyle: 1
                ];

                let action: ObjcId = msg_send![
                    class!(UIAlertAction),
                    actionWithTitle: str_to_ns("OK")
                    style: 0
                    handler: ConcreteBlock::new({
                        let alert = alert; // TODO strong ptr?
                        move |_: ObjcId| {
                            let fields: ObjcId = msg_send![alert, textFields];
                            let field: ObjcId = msg_send![fields, firstObject];
                            let text: *const NSString = msg_send![field, text];
                            *INPUT_TEXT.lock().unwrap() = Some((*text).as_str().to_owned());
                        }
                    }).copy()
                ];
                let _: () = msg_send![alert, addAction: action];

                let text = text.to_owned();
                let _: () = msg_send![alert, addTextFieldWithConfigurationHandler: ConcreteBlock::new(move |field: ObjcId| {
                    let _: () = msg_send![field, setPlaceholder: str_to_ns("文字")];
                    let _: () = msg_send![field, setText: str_to_ns(&text)];
                }).copy()];

                let _: () = msg_send![
                    view_ctrl as ObjcId,
                    presentViewController: alert
                    animated: runtime::YES
                    completion: 0 as ObjcId
                ];
                show_message(&format!("哈哈 {}", 123));
            }
        } else {
            *INPUT_TEXT.lock().unwrap() = Some(unsafe { get_internal_gl() }.quad_context.clipboard_get().unwrap_or_default());
            show_message("从剪贴板加载成功");
        }
    }
}

pub fn take_input() -> Option<(String, String)> {
    INPUT_TEXT
        .lock()
        .unwrap()
        .take()
        .map(|text| (CURRENT_INPUT.with(|it| std::mem::take(it.borrow_mut().deref_mut())), text))
}

pub fn return_input(id: String, text: String) {
    CURRENT_INPUT.with(|it| *it.borrow_mut() = id);
    *INPUT_TEXT.lock().unwrap() = Some(text);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn request_file(id: impl Into<String>) {
    CURRENT_CHOOSE_FILE.with(|it| *it.borrow_mut() = id.into());
    *CHOSEN_FILE.lock().unwrap() = None;
    cfg_if! {
        if #[cfg(target_os = "android")] {
            unsafe {
                let env = miniquad::native::attach_jni_env();
                let ctx = ndk_context::android_context().context();
                let class = (**env).GetObjectClass.unwrap()(env, ctx);
                let method = (**env).GetMethodID.unwrap()(env, class, b"chooseFile\0".as_ptr() as _, b"()V\0".as_ptr() as _);
                (**env).CallVoidMethod.unwrap()(env, ctx, method);
            }
        } else if #[cfg(target_os = "ios")] {
            use once_cell::sync::Lazy;
            unsafe {
                use crate::objc::*;
                static PICKER_DELEGATE: Lazy<u64> = Lazy::new(|| unsafe {
                    let mut decl = ClassDecl::new("PickerDelegate", class!(NSObject)).unwrap();
                    extern "C" fn document_picker(_: &Object, _: Sel, _: ObjcId, documents: ObjcId) {
                        unsafe {
                            let url: ObjcId = msg_send![documents, firstObject];
                            let data: ObjcId = msg_send![class!(NSData), dataWithContentsOfURL: url];
                            if data.is_null() {
                                show_message("读取文件失败");
                            } else {
                                extern "C" {
                                    #[allow(improper_ctypes)]
                                    pub fn NSTemporaryDirectory() -> *mut NSString;
                                }
                                let dir = NSTemporaryDirectory();
                                let uuid: ObjcId = msg_send![class!(NSUUID), UUID];
                                let uuid: *mut NSString = msg_send![uuid, UUIDString];
                                let path = format!("{}{}", (*dir).as_str(), (*uuid).as_str());
                                let _: () = msg_send![data, writeToFile: str_to_ns(&path) atomically: YES];
                                *CHOSEN_FILE.lock().unwrap() = Some(path);
                            }
                        }
                    }
                    decl.add_method(sel!(documentPicker: didPickDocumentsAtURLs:), document_picker as extern "C" fn(&Object, Sel, ObjcId, ObjcId));
                    decl.register() as *const _ as _
                });

                let picker: ObjcId = msg_send![class!(UIDocumentPickerViewController), alloc];
                let picker: ObjcId = if available("14.0.0") {
                    let tp_cls = class!(UTType);
                    let ext = |e: &str| {
                        let tp: ObjcId = msg_send![tp_cls, typeWithFilenameExtension: str_to_ns(e)];
                        std::mem::transmute::<_, ShareId<NSObject>>(ShareId::from_ptr(tp))
                    };
                    let types = NSArray::from_slice(&[ext("zip"), ext("pez")]);
                    let types: ObjcId = std::mem::transmute(types);
                    msg_send![picker, initForOpeningContentTypes: types]
                } else {
                    let ext = |e: &str| str_to_ns(e);
                    let types = NSArray::from_vec(vec![ext("zip"), ext("pez")]);
                    let types: ObjcId = std::mem::transmute(types);
                    msg_send![picker, documentTypes: types inMode: 0]
                };
                let dlg_obj: ObjcId = msg_send![*PICKER_DELEGATE as ObjcId, alloc];
                let dlg_obj: ObjcId = msg_send![dlg_obj, init];
                let _: () = msg_send![picker, setDelegate: dlg_obj];

                let view_ctrl = *miniquad::native::ios::VIEW_CTRL_OBJ.lock().unwrap();
                let _: () = msg_send![
                    view_ctrl as ObjcId,
                    presentViewController: picker
                    animated: runtime::YES
                    completion: 0 as ObjcId
                ];
            }
        } else {
            *CHOSEN_FILE.lock().unwrap() = rfd::FileDialog::new().pick_file().map(|it| it.display().to_string());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn take_file() -> Option<(String, String)> {
    CHOSEN_FILE
        .lock()
        .unwrap()
        .take()
        .map(|file| (CURRENT_CHOOSE_FILE.with(|it| std::mem::take(it.borrow_mut().deref_mut())), file))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn return_file(id: String, file: String) {
    CURRENT_CHOOSE_FILE.with(|it| *it.borrow_mut() = id);
    *CHOSEN_FILE.lock().unwrap() = Some(file);
}

pub trait Scene {
    fn enter(&mut self, _tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        Ok(())
    }
    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }
    fn touch(&mut self, _tm: &mut TimeManager, _touch: &Touch) -> Result<bool> {
        Ok(false)
    }
    fn update(&mut self, tm: &mut TimeManager) -> Result<()>;
    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()>;
    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        NextScene::None
    }
}

pub trait RenderTargetChooser {
    fn choose(&mut self) -> Option<RenderTarget>;
}
impl RenderTargetChooser for Option<RenderTarget> {
    fn choose(&mut self) -> Option<RenderTarget> {
        *self
    }
}
impl<F: FnMut() -> Option<RenderTarget>> RenderTargetChooser for F {
    fn choose(&mut self) -> Option<RenderTarget> {
        self()
    }
}

pub struct Main {
    pub scenes: Vec<Box<dyn Scene>>,
    times: Vec<f64>,
    target_chooser: Box<dyn RenderTargetChooser>,
    tm: TimeManager,
    paused: bool,
    last_update_time: f64,
    should_exit: bool,
    pub show_billboard: bool,
    touches: Option<Vec<Touch>>,
}

impl Main {
    pub fn new(mut scene: Box<dyn Scene>, mut tm: TimeManager, mut target_chooser: impl RenderTargetChooser + 'static) -> Result<Self> {
        simulate_mouse_with_touch(false);
        scene.enter(&mut tm, target_chooser.choose())?;
        let last_update_time = tm.now();
        Ok(Self {
            scenes: vec![scene],
            times: Vec::new(),
            target_chooser: Box::new(target_chooser),
            tm,
            paused: false,
            last_update_time,
            should_exit: false,
            show_billboard: true,
            touches: None,
        })
    }

    pub fn update(&mut self) -> Result<()> {
        self.update_with_mutate(|_| {})
    }

    pub fn update_with_mutate(&mut self, f: impl Fn(&mut Touch)) -> Result<()> {
        if self.paused {
            return Ok(());
        }
        match self.scenes.last_mut().unwrap().next_scene(&mut self.tm) {
            NextScene::None => {}
            NextScene::Pop => {
                self.scenes.pop();
                self.tm.seek_to(self.times.pop().unwrap());
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target_chooser.choose())?;
            }
            NextScene::PopN(num) => {
                for _ in 0..num {
                    self.scenes.pop();
                    self.tm.seek_to(self.times.pop().unwrap());
                }
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target_chooser.choose())?;
            }
            NextScene::Exit => {
                self.should_exit = true;
            }
            NextScene::Overlay(mut scene) => {
                self.times.push(self.tm.now());
                scene.enter(&mut self.tm, self.target_chooser.choose())?;
                self.scenes.push(scene);
            }
            NextScene::Replace(mut scene) => {
                scene.enter(&mut self.tm, self.target_chooser.choose())?;
                *self.scenes.last_mut().unwrap() = scene;
            }
        }
        Judge::on_new_frame();
        let mut touches = Judge::get_touches();
        touches.iter_mut().for_each(f);
        if !touches.is_empty() {
            let now = self.tm.now();
            let delta = (now - self.last_update_time) / touches.len() as f64;
            DIALOG.with(|it| -> Result<()> {
                let mut index = 1;
                touches.retain_mut(|touch| {
                    let t = self.last_update_time + (index + 1) as f64 * delta;
                    index += 1;
                    let mut guard = it.borrow_mut();
                    if let Some(dialog) = guard.as_mut() {
                        if !dialog.touch(&touch, t as _) {
                            drop(guard);
                            *it.borrow_mut() = None;
                        }
                        false
                    } else {
                        drop(guard);
                        self.tm.seek_to(t);
                        !self.scenes.last_mut().unwrap().touch(&mut self.tm, touch).unwrap_or(false)
                    }
                });
                Ok(())
            })?;
            self.tm.seek_to(now);
        }
        self.touches = Some(touches);
        self.last_update_time = self.tm.now();
        DIALOG.with(|it| {
            if let Some(dialog) = it.borrow_mut().as_mut() {
                dialog.update(self.last_update_time as _);
            }
        });
        self.scenes.last_mut().unwrap().update(&mut self.tm)
    }

    pub fn render(&mut self, ui: &mut Ui) -> Result<()> {
        if self.paused {
            return Ok(());
        }
        ui.set_touches(self.touches.take().unwrap());
        ui.scope(|ui| self.scenes.last_mut().unwrap().render(&mut self.tm, ui))?;
        if self.show_billboard {
            BILLBOARD.with(|it| {
                let mut guard = it.borrow_mut();
                let t = guard.1.now() as f32;
                guard.0.render(ui, t);
            });
        }
        DIALOG.with(|it| {
            if let Some(dialog) = it.borrow_mut().as_mut() {
                dialog.render(ui);
            }
        });
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.paused = true;
        self.scenes.last_mut().unwrap().pause(&mut self.tm)
    }

    pub fn resume(&mut self) -> Result<()> {
        self.paused = false;
        self.scenes.last_mut().unwrap().resume(&mut self.tm)
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }
}

fn draw_background(tex: Texture2D) {
    let asp = screen_aspect();
    let top = 1. / asp;
    draw_image(tex, Rect::new(-1., -top, 2., top * 2.), ScaleType::Scale);
    draw_rectangle(-1., -top, 2., top * 2., Color::new(0., 0., 0., 0.3));
}

fn draw_illustration(tex: Texture2D, x: f32, y: f32, w: f32, h: f32, color: Color) -> Rect {
    let scale = 0.076;
    let w = scale * 13. * w;
    let h = scale * 7. * h;
    let r = Rect::new(x - w / 2., y - h / 2., w, h);
    let tr = {
        let exp = w / h;
        let act = tex.width() / tex.height();
        if exp > act {
            let h = act / exp;
            Rect::new(0., 0.5 - h / 2., 1., h)
        } else {
            let w = exp / act;
            Rect::new(0.5 - w / 2., 0., w, 1.)
        }
    };
    crate::ext::draw_parallelogram(r, Some((tex, tr)), color, true);
    r
}
