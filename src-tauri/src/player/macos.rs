//! macOS video surface. mpv's own macOS output ignores `--wid`, so we use the libmpv
//! render API: mpv draws each frame into a CAOpenGLLayer that sits below the webview.
#![allow(deprecated)] // Apple marks OpenGL deprecated; it is still the only GPU API the mpv render API supports.

use std::ffi::{c_void, CString};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use libmpv2::Mpv;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindowOrderingMode};
use objc2_core_foundation::CFTimeInterval;
use objc2_core_video::CVTimeStamp;
use objc2_open_gl::{
    CGLChoosePixelFormat, CGLContextObj, CGLOpenGLProfile, CGLPixelFormatAttribute, CGLPixelFormatObj,
    CGLSetCurrentContext,
};
use objc2_quartz_core::{CALayer, CAOpenGLLayer};

const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_VIEWPORT: u32 = 0x0BA2;
type GLint = i32;
const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

#[link(name = "OpenGL", kind = "framework")]
extern "C" {
    fn glGetIntegerv(pname: u32, data: *mut GLint);
}

extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
}

/// The render context is only touched from CoreAnimation's draw callbacks, which never overlap.
struct Renderer(RenderContext<'static>);
unsafe impl Send for Renderer {}

static MPV: OnceLock<Arc<Mpv>> = OnceLock::new();
static RENDERER: Mutex<Option<Renderer>> = Mutex::new(None);
static NEEDS_DRAW: AtomicBool = AtomicBool::new(true);
static RENDERER_READY: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Hands the started mpv core to the video layer and waits until the layer has created
/// its render context. mpv with `vo=libmpv` turns video off for files loaded before that.
pub fn attach_mpv(mpv: Arc<Mpv>, timeout: Duration) -> Result<(), String> {
    let _ = MPV.set(mpv);
    NEEDS_DRAW.store(true, Ordering::Release);

    let (lock, ready) = &RENDERER_READY;
    let guard = lock.lock().map_err(|_| "renderer state is poisoned")?;
    let (guard, _) =
        ready.wait_timeout_while(guard, timeout, |created| !*created).map_err(|_| "renderer state is poisoned")?;
    if *guard {
        Ok(())
    } else {
        Err("the video layer did not start; is the window visible?".into())
    }
}

fn get_proc_address(_ctx: &(), name: &str) -> *mut c_void {
    match CString::new(name) {
        Ok(symbol) => unsafe { dlsym(RTLD_DEFAULT, symbol.as_ptr()) },
        Err(_) => std::ptr::null_mut(),
    }
}

fn create_renderer(mpv: &'static Mpv) -> Option<Renderer> {
    let params = [
        RenderParam::ApiType(RenderParamApiType::OpenGl),
        RenderParam::InitParams(OpenGLInitParams { get_proc_address, ctx: () }),
    ];
    let mut ctx = mpv.create_render_context(params).ok()?;
    ctx.set_update_callback(|| NEEDS_DRAW.store(true, Ordering::Release));
    Some(Renderer(ctx))
}

define_class!(
    #[unsafe(super(CAOpenGLLayer, CALayer, NSObject))]
    #[name = "LokiiVideoLayer"]
    struct VideoLayer;

    impl VideoLayer {
        #[unsafe(method(canDrawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn can_draw(
            &self,
            _ctx: CGLContextObj,
            _pf: CGLPixelFormatObj,
            _t: CFTimeInterval,
            _ts: *const CVTimeStamp,
        ) -> bool {
            MPV.get().is_some() && NEEDS_DRAW.load(Ordering::Acquire)
        }

        #[unsafe(method(drawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn draw(
            &self,
            ctx: CGLContextObj,
            pf: CGLPixelFormatObj,
            t: CFTimeInterval,
            ts: *const CVTimeStamp,
        ) {
            NEEDS_DRAW.store(false, Ordering::Release);
            unsafe { CGLSetCurrentContext(ctx) };
            render_frame();
            // The superclass flushes the frame to the screen.
            let _: () = unsafe {
                msg_send![super(self), drawInCGLContext: ctx, pixelFormat: pf, forLayerTime: t, displayTime: ts]
            };
        }

        #[unsafe(method(copyCGLPixelFormatForDisplayMask:))]
        fn copy_pixel_format(&self, mask: u32) -> CGLPixelFormatObj {
            core_profile_pixel_format().unwrap_or_else(|| unsafe {
                msg_send![super(self), copyCGLPixelFormatForDisplayMask: mask]
            })
        }
    }
);

fn render_frame() {
    let Some(mpv) = MPV.get() else { return };
    let Ok(mut guard) = RENDERER.lock() else {
        return;
    };
    if guard.is_none() {
        *guard = create_renderer(mpv.as_ref());
        if guard.is_some() {
            let (lock, ready) = &RENDERER_READY;
            if let Ok(mut created) = lock.lock() {
                *created = true;
                ready.notify_all();
            }
        }
    }
    let Some(Renderer(ctx)) = guard.as_ref() else {
        return;
    };

    let mut fbo: GLint = 0;
    let mut viewport: [GLint; 4] = [0; 4];
    unsafe {
        glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &mut fbo);
        glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr());
    }
    let _ = ctx.update();
    let _ = ctx.render::<()>(fbo, viewport[2], viewport[3], true);
}

/// OpenGL 3.2 core profile, which mpv needs for hardware-decoded (VideoToolbox) frames.
fn core_profile_pixel_format() -> Option<CGLPixelFormatObj> {
    let mut attribs = [
        CGLPixelFormatAttribute::CGLPFAOpenGLProfile,
        CGLPixelFormatAttribute(CGLOpenGLProfile::CGLOGLPVersion_GL3_Core.0),
        CGLPixelFormatAttribute::CGLPFAAccelerated,
        CGLPixelFormatAttribute::CGLPFADoubleBuffer,
        CGLPixelFormatAttribute::CGLPFAColorSize,
        CGLPixelFormatAttribute(24),
        CGLPixelFormatAttribute::CGLPFAAlphaSize,
        CGLPixelFormatAttribute(8),
        CGLPixelFormatAttribute::CGLPFASupportsAutomaticGraphicsSwitching,
        CGLPixelFormatAttribute(0),
    ];
    let mut pix: CGLPixelFormatObj = std::ptr::null_mut();
    let mut count: GLint = 0;
    let err = unsafe {
        CGLChoosePixelFormat(NonNull::new(attribs.as_mut_ptr())?, NonNull::from(&mut pix), NonNull::from(&mut count))
    };
    (err.0 == 0 && !pix.is_null()).then_some(pix)
}

/// Adds a layer-hosting view below the webview. The transparent webview draws the controls on top.
pub fn attach_video_view(window: &tauri::WebviewWindow) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("video view must be created on the main thread")?;
    let content_ptr = window.ns_view().map_err(|e| e.to_string())? as *const NSView;
    // SAFETY: Tauri returns the window's content view, which lives as long as the window.
    let content = unsafe { content_ptr.as_ref() }.ok_or("window has no content view")?;
    let scale = content.window().map(|w| w.backingScaleFactor()).unwrap_or(2.0);

    let layer: Retained<VideoLayer> = unsafe { msg_send![VideoLayer::alloc(), init] };
    layer.setAsynchronous(true);
    layer.setNeedsDisplayOnBoundsChange(true);
    layer.setContentsScale(scale);

    let video = NSView::initWithFrame(mtm.alloc(), content.bounds());
    video.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    video.setLayer(Some(&layer));
    video.setWantsLayer(true);
    content.addSubview_positioned_relativeTo(&video, NSWindowOrderingMode::Below, None);
    Ok(())
}
