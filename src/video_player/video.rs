use crate::video_player::Error;
use libmpv2::Mpv;
use libmpv2_sys::*;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, HANDLE, HINSTANCE, LPARAM, LRESULT, WPARAM, HMODULE};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11Texture2D, D3D11_TEXTURE2D_DESC, D3D11_BIND_SHADER_RESOURCE,
    D3D11_BIND_RENDER_TARGET, D3D11_USAGE_DEFAULT, D3D11_RESOURCE_MISC_SHARED,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::OpenGL::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{w, Interface, PCSTR};
use std::ffi::{CString, c_void};
use std::os::raw::c_char;
use std::sync::Once;
use gpui::RenderImage;

static REGISTER_DUMMY_CLASS: Once = Once::new();
static mut OPENGL32_DLL: HMODULE = HMODULE(std::ptr::null_mut());

// OpenGL Constants
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const WGL_ACCESS_READ_WRITE_NV: u32 = 0x0001;

type GlGenFramebuffers = unsafe extern "system" fn(n: i32, framebuffers: *mut u32);
type GlBindFramebuffer = unsafe extern "system" fn(target: u32, framebuffer: u32);
type GlFramebufferTexture2D = unsafe extern "system" fn(target: u32, attachment: u32, textarget: u32, texture: u32, level: i32);
type GlDeleteFramebuffers = unsafe extern "system" fn(n: i32, framebuffers: *const u32);

type WglDXOpenDeviceNV = unsafe extern "system" fn(dx_device: *mut c_void) -> HANDLE;
type WglDXCloseDeviceNV = unsafe extern "system" fn(device: HANDLE) -> bool;
type WglDXRegisterObjectNV = unsafe extern "system" fn(device: HANDLE, dx_resource: *mut c_void, name: u32, type_: u32, access: u32) -> HANDLE;
type WglDXUnregisterObjectNV = unsafe extern "system" fn(device: HANDLE, object: HANDLE) -> bool;
type WglDXLockObjectsNV = unsafe extern "system" fn(device: HANDLE, count: i32, objects: *mut HANDLE) -> bool;
type WglDXUnlockObjectsNV = unsafe extern "system" fn(device: HANDLE, count: i32, objects: *mut HANDLE) -> bool;

pub(crate) struct GlProcs {
    gen_framebuffers: GlGenFramebuffers,
    bind_framebuffer: GlBindFramebuffer,
    framebuffer_texture2d: GlFramebufferTexture2D,
    delete_framebuffers: GlDeleteFramebuffers,
}

pub(crate) struct Interop {
    open_device: WglDXOpenDeviceNV,
    close_device: WglDXCloseDeviceNV,
    register_object: WglDXRegisterObjectNV,
    unregister_object: WglDXUnregisterObjectNV,
    lock_objects: WglDXLockObjectsNV,
    unlock_objects: WglDXUnlockObjectsNV,
}

#[derive(Clone, Copy, Debug)]
pub struct SendHandle(pub HANDLE);
unsafe impl Send for SendHandle {}
unsafe impl Sync for SendHandle {}

#[derive(Debug, Clone)]
pub struct VideoOptions {
    pub frame_buffer_capacity: Option<usize>,
    pub looping: Option<bool>,
    pub speed: Option<f64>,
    pub source_name: Option<String>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self { frame_buffer_capacity: Some(3), looping: Some(false), speed: Some(1.0), source_name: None }
    }
}

pub(crate) struct Internal {
    pub(crate) mpv: Mpv,
    pub(crate) render_context: *mut mpv_render_context,
    pub(crate) gl_context: HGLRC,
    pub(crate) h_dc: HDC,
    pub(crate) dummy_hwnd: HWND,
    pub(crate) interop: Interop,
    pub(crate) gl_procs: GlProcs,
    pub(crate) interop_device: HANDLE,
    pub(crate) interop_texture_handle: HANDLE,
    pub(crate) d3d_texture: ID3D11Texture2D,
    pub(crate) gl_texture: u32,
    pub(crate) gl_fbo: u32,
    pub(crate) render_image: Arc<RenderImage>,
    pub(crate) upload_frame: Arc<AtomicBool>,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) source_name: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) display_width_override: Option<u32>,
    pub(crate) display_height_override: Option<u32>,
}

impl std::fmt::Debug for Internal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Internal").field("source_name", &self.source_name).finish()
    }
}

unsafe impl Send for Internal {}
unsafe impl Sync for Internal {}

#[derive(Debug, Clone)]
pub struct Video(pub(crate) Arc<RwLock<Internal>>);

impl Video {
    pub fn new_with_options(
        path: &str, 
        options: VideoOptions, 
        d3d11_device_ptr: Option<*mut std::ffi::c_void>
    ) -> Result<Self, Error> {
        unsafe {
            if OPENGL32_DLL.0.is_null() {
                OPENGL32_DLL = windows::Win32::System::LibraryLoader::LoadLibraryW(w!("opengl32.dll")).map_err(|e| Error::OpenGL(format!("LoadLibrary opengl32.dll failed: {:?}", e)))?;
            }

            // The D3D11 device in AppState is AddRef'd and outlives all Video instances.
            let d3d_device: ID3D11Device = std::mem::transmute_copy(&d3d11_device_ptr.ok_or(Error::Lock)?);
            
            unsafe extern "system" fn dummy_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }

            let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).map_err(|e| Error::Mpv(format!("GetModuleHandleW failed: {:?}", e)))?;
            let class_name = w!("LumaGLDummyClass");
            
            REGISTER_DUMMY_CLASS.call_once(|| {
                let wnd_class = WNDCLASSW {
                    lpfnWndProc: Some(dummy_wnd_proc),
                    hInstance: HINSTANCE(instance.0),
                    lpszClassName: class_name,
                    ..Default::default()
                };
                RegisterClassW(&wnd_class);
            });

            let dummy_hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(), class_name, w!("GLDummy"), WS_POPUP,
                0, 0, 1, 1, None, None, Some(instance.into()), None
            ).map_err(|e| Error::OpenGL(format!("CreateWindowExW failed: {:?}", e)))?;

            let h_dc = GetDC(Some(dummy_hwnd));
            let pfd = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 32,
                ..Default::default()
            };
            let pixel_format = ChoosePixelFormat(h_dc, &pfd);
            if pixel_format == 0 { return Err(Error::OpenGL("ChoosePixelFormat failed".into())); }
            let _ = SetPixelFormat(h_dc, pixel_format, &pfd);
            
            let gl_context = wglCreateContext(h_dc).map_err(|e| Error::OpenGL(format!("wglCreateContext failed: {:?}", e)))?;
            wglMakeCurrent(h_dc, gl_context).ok();

            let load_ext = |name: &str| -> Result<*const c_void, Error> {
                let cname = CString::new(name).map_err(|_| Error::Interop(format!("Invalid extension name: {}", name)))?;
                let addr = wglGetProcAddress(PCSTR(cname.as_ptr() as *const u8));
                if let Some(p) = addr {
                    return Ok(p as *const c_void);
                }
                let p = windows::Win32::System::LibraryLoader::GetProcAddress(OPENGL32_DLL, PCSTR(cname.as_ptr() as *const u8));
                match p {
                    Some(p) => Ok(p as *const c_void),
                    None => Err(Error::Interop(format!("Extension function not found: {}", name))),
                }
            };

            let gl_procs = GlProcs {
                gen_framebuffers: std::mem::transmute(load_ext("glGenFramebuffers")?),
                bind_framebuffer: std::mem::transmute(load_ext("glBindFramebuffer")?),
                framebuffer_texture2d: std::mem::transmute(load_ext("glFramebufferTexture2D")?),
                delete_framebuffers: std::mem::transmute(load_ext("glDeleteFramebuffers")?),
            };

            let interop = Interop {
                open_device: std::mem::transmute(load_ext("wglDXOpenDeviceNV")?),
                close_device: std::mem::transmute(load_ext("wglDXCloseDeviceNV")?),
                register_object: std::mem::transmute(load_ext("wglDXRegisterObjectNV")?),
                unregister_object: std::mem::transmute(load_ext("wglDXUnregisterObjectNV")?),
                lock_objects: std::mem::transmute(load_ext("wglDXLockObjectsNV")?),
                unlock_objects: std::mem::transmute(load_ext("wglDXUnlockObjectsNV")?),
            };

            let interop_device = (interop.open_device)(d3d_device.as_raw());
            if interop_device.is_invalid() { return Err(Error::Interop("wglDXOpenDeviceNV failed".into())); }

            let width = 2560; let height = 1440;
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width, Height: height, MipLevels: 1, ArraySize: 1, Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0, MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
            };
            let mut d3d_texture = None;
            d3d_device.CreateTexture2D(&desc, None, Some(&mut d3d_texture)).map_err(|_| Error::Lock)?;
            let d3d_texture = d3d_texture.ok_or_else(|| Error::OpenGL("CreateTexture2D returned None".into()))?;

            let mut gl_texture = 0;
            glGenTextures(1, &mut gl_texture);
            let interop_texture_handle = (interop.register_object)(
                interop_device, d3d_texture.as_raw(), gl_texture, GL_TEXTURE_2D, WGL_ACCESS_READ_WRITE_NV
            );
            if interop_texture_handle.is_invalid() { return Err(Error::Interop("wglDXRegisterObjectNV failed".into())); }

            let mut gl_fbo = 0;
            (gl_procs.gen_framebuffers)(1, &mut gl_fbo);
            (gl_procs.bind_framebuffer)(GL_FRAMEBUFFER, gl_fbo);
            (gl_procs.framebuffer_texture2d)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, gl_texture, 0);

            let mpv = Mpv::new().map_err(|e| Error::Mpv(format!("Mpv::new failed: {:?}", e)))?;

            mpv.set_property("vo", "libmpv").ok();
            mpv.set_property("gpu-api", "opengl").ok();
            mpv.set_property("hwdec", "auto-safe").ok(); // D3D11 is safest for Windows

            if let Some(ptr) = d3d11_device_ptr {
                let ptr_str = format!("{}", ptr as usize);
                let _ = mpv.set_property("d3d11-device", &*ptr_str);
            }

            // Force strict timestamp regeneration
            mpv.set_property("demuxer-lavf-o", "fflags=+genpts").ok();

            mpv.set_property("keep-open", "yes").ok();

            mpv.set_property("interpolation", "yes").ok();
            mpv.set_property("tscale", "oversample").ok();
            mpv.set_property("prefetch-playlist", "yes").ok();
            mpv.set_property("cache", "yes").ok();
            mpv.set_property("demuxer-thread", "yes").ok();
            mpv.set_property("demuxer-readahead-secs", "5").ok();
            mpv.set_property("demuxer-max-bytes", "500000000").ok();
            mpv.set_property("demuxer-max-back-bytes", "100000000").ok();
            mpv.set_property("swapchain-depth", "3").ok();
            mpv.set_property("gpu-dumb-mode", "no").ok();
            mpv.set_property("vd-lavc-threads", "4").ok();
            mpv.set_property("hr-seek-framedrop", "no").ok();

            // Send auth token as HTTP header so the local HLS server can
            // authenticate both playlist and segment requests.
            let token_header = format!("X-Luma-Token: {}", crate::get_hls_token());
            mpv.set_property("http-header-fields", &*token_header).ok();

            let mut render_context: *mut mpv_render_context = std::ptr::null_mut();
            
            unsafe extern "C" fn get_proc_address_callback(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
                let cname = std::ffi::CStr::from_ptr(name);
                let addr = wglGetProcAddress(PCSTR(cname.as_ptr() as *const u8));
                if let Some(p) = addr { return p as *mut c_void; }
                windows::Win32::System::LibraryLoader::GetProcAddress(OPENGL32_DLL, PCSTR(cname.as_ptr() as *const u8)).map_or(std::ptr::null_mut(), |p| p as *mut c_void)
            }

            let api_type = CString::new("opengl").expect("static string");
            let mut params = [
                mpv_render_param { type_: mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE, data: api_type.as_ptr() as *mut _ },
                mpv_render_param { type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, data: &mut mpv_opengl_init_params {
                    get_proc_address: Some(get_proc_address_callback), get_proc_address_ctx: std::ptr::null_mut(),
                } as *mut _ as *mut _ },
                mpv_render_param { type_: mpv_render_param_type_MPV_RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
            ];

            let err = mpv_render_context_create(&mut render_context, mpv.ctx.as_ptr(), params.as_mut_ptr());
            if err < 0 { return Err(Error::Mpv(format!("mpv_render_context_create failed: {}", err))); }

            let placeholder = image::ImageBuffer::new(1, 1);
            let render_image = Arc::new(RenderImage::new(vec![image::Frame::new(placeholder)]));
            let upload_frame = Arc::new(AtomicBool::new(false));

            let alive = Arc::new(AtomicBool::new(true));
            let alive_ref = Arc::clone(&alive);
            let mpv_ctx_ptr = mpv.ctx.as_ptr() as usize;
            let worker = std::thread::spawn(move || {
                let ctx = mpv_ctx_ptr as *mut mpv_handle;
                while alive_ref.load(Ordering::Acquire) {
                    let event = mpv_wait_event(ctx, 0.1); // Increased timeout to 100ms
                    if event.is_null() || (*event).event_id == mpv_event_id_MPV_EVENT_NONE { 
                        std::thread::sleep(Duration::from_millis(10)); // Explicit safety sleep
                        continue; 
                    }
                    if (*event).event_id == mpv_event_id_MPV_EVENT_SHUTDOWN { break; }
                }
            });

            mpv.command("loadfile", &[path, "replace"]).ok();

            Ok(Video(Arc::new(RwLock::new(Internal {
                mpv, render_context, gl_context, h_dc, dummy_hwnd, interop, gl_procs, interop_device,
                interop_texture_handle, d3d_texture, gl_texture, gl_fbo, render_image, upload_frame, alive,
                worker: Some(worker), source_name: options.source_name.unwrap_or_default(), width, height,
                display_width_override: None, display_height_override: None,
            }))))
        }
    }

    pub fn render_to_texture(&self) {
        let inner = self.0.read();
        unsafe {
            // OPTIMIZATION: Only perform GL work if MPV says there's a new frame or OSD update
            let flags = mpv_render_context_update(inner.render_context);
            if (flags & mpv_render_update_flag_MPV_RENDER_UPDATE_FRAME as u64) == 0 {
                return;
            }

            if wglMakeCurrent(inner.h_dc, inner.gl_context).is_err() {
                return;
            }
            
            let mut handle = inner.interop_texture_handle;
            if !(inner.interop.lock_objects)(inner.interop_device, 1, &mut handle) {
                return;
            }

            let mut fbo = mpv_opengl_fbo { fbo: inner.gl_fbo as i32, w: inner.width as i32, h: inner.height as i32, internal_format: 0 };
            let mut params = [
                mpv_render_param { type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO, data: &mut fbo as *mut _ as *mut _ },
                mpv_render_param { type_: mpv_render_param_type_MPV_RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
            ];
            
            if mpv_render_context_render(inner.render_context, params.as_mut_ptr()) >= 0 {
                inner.upload_frame.store(true, Ordering::SeqCst);
            }
            
            (inner.interop.unlock_objects)(inner.interop_device, 1, &mut handle);
        }
    }
}

impl Drop for Internal {
    fn drop(&mut self) {
        // 1. Signal worker to stop
        self.alive.store(false, Ordering::Release);
        
        // 2. Wait for worker to exit before cleanup
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }

        unsafe {
            // 3. Perform GL cleanup
            if wglMakeCurrent(self.h_dc, self.gl_context).is_ok() {
                (self.interop.unregister_object)(self.interop_device, self.interop_texture_handle);
                (self.interop.close_device)(self.interop_device);
                (self.gl_procs.delete_framebuffers)(1, &self.gl_fbo);
                glDeleteTextures(1, &self.gl_texture);
                wglMakeCurrent(self.h_dc, HGLRC(std::ptr::null_mut())).ok();
            }
            
            // 4. Free MPV context
            mpv_render_context_free(self.render_context);
            wglDeleteContext(self.gl_context).ok();
            ReleaseDC(Some(self.dummy_hwnd), self.h_dc);
            let _ = DestroyWindow(self.dummy_hwnd);
        }
    }
}

impl Video {
    pub(crate) fn read(&'_ self) -> parking_lot::RwLockReadGuard<'_, Internal> { self.0.read() }
    pub fn size(&self) -> (i32, i32) { let inner = self.read(); (inner.width as i32, inner.height as i32) }
    pub fn display_size(&self) -> (u32, u32) {
        let inner = self.read();
        let (nw, nh) = (inner.width, inner.height);
        let ar = if nh == 0 { 1.0 } else { nw as f32 / nh as f32 };
        match (inner.display_width_override, inner.display_height_override) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, if ar == 0.0 { nh } else { (w as f32 / ar).round() as u32 }),
            (None, Some(h)) => (((h as f32) * ar).round() as u32, h),
            (None, None) => (nw, nh),
        }
    }

    pub fn eof_reached(&self) -> bool {
        self.read().mpv.get_property::<bool>("eof-reached").unwrap_or(false)
    }

    pub fn aspect_ratio(&self) -> f32 { let (w, h) = self.size(); w as f32 / h as f32 }
    pub fn set_paused(&self, p: bool) { let _ = self.read().mpv.set_property("pause", p); }
    pub fn paused(&self) -> bool { self.read().mpv.get_property::<bool>("pause").unwrap_or(true) }
    pub fn position(&self) -> Duration { Duration::from_secs_f64(self.read().mpv.get_property::<f64>("time-pos").unwrap_or(0.0)) }
    pub fn duration(&self) -> Duration { Duration::from_secs_f64(self.read().mpv.get_property::<f64>("duration").unwrap_or(0.0)) }
    pub fn take_frame_ready(&self) -> bool { self.render_to_texture(); self.read().upload_frame.swap(false, Ordering::SeqCst) }
    pub fn set_display_size(&self, w: Option<u32>, h: Option<u32>) { let mut inner = self.0.write(); inner.display_width_override = w; inner.display_height_override = h; }

        pub fn load_file(&self, path: &str) -> Result<(), Error> {
            let inner = self.read();
            inner.mpv.command("loadfile", &[path, "replace"]).map_err(|_| Error::Bus)
        }
        pub fn seek(&self, pos: Duration, _: bool) -> Result<(), Error> { 
     self.read().mpv.command("seek", &[&format!("{}", pos.as_secs_f64()), "absolute"]).map_err(|_| Error::Bus) }
    pub fn buffered_len(&self) -> usize { 0 }
    pub fn set_frame_buffer_capacity(&self, _: usize) {}
    pub fn set_volume(&self, volume: f64) {
        let _ = self.read().mpv.set_property("volume", volume);
    }

    /// Returns list of audio tracks: (track_id, title/label)
    pub fn audio_tracks(&self) -> Vec<(i64, String)> {
        let inner = self.read();
        let count = inner.mpv.get_property::<i64>("track-list/count").unwrap_or(0);
        let mut tracks = Vec::new();
        for i in 0..count {
            let track_type = inner.mpv.get_property::<String>(&format!("track-list/{}/type", i)).unwrap_or_default();
            if track_type != "audio" { continue; }
            let id = inner.mpv.get_property::<i64>(&format!("track-list/{}/id", i)).unwrap_or(0);
            let title = inner.mpv.get_property::<String>(&format!("track-list/{}/title", i)).unwrap_or_default();
            let lang = inner.mpv.get_property::<String>(&format!("track-list/{}/lang", i)).unwrap_or_default();
            let label = if !title.is_empty() {
                title
            } else if !lang.is_empty() {
                format!("Track {} ({})", id, lang)
            } else {
                format!("Track {}", id)
            };
            tracks.push((id, label));
        }
        tracks
    }

    /// Get currently active audio track ID (0 = none/disabled)
    pub fn current_audio_track(&self) -> i64 {
        self.read().mpv.get_property::<i64>("aid").unwrap_or(1)
    }

    /// Set active audio track by ID (0 to disable audio)
    pub fn set_audio_track(&self, id: i64) {
        let _ = self.read().mpv.set_property("aid", id);
    }
}
