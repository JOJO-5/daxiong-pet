//! LiteRT-LM 0.18.0 opaque C ABI, checked against c/engine.h and
//! c/conversation.h. No Python, no C++ object layouts cross this boundary.
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::sync::Arc;

type Ptr = *mut c_void;
type New = unsafe extern "C" fn() -> Ptr;
type Delete = unsafe extern "C" fn(Ptr);
type SetInt = unsafe extern "C" fn(Ptr, c_int);
type SetPtr = unsafe extern "C" fn(Ptr, Ptr);
type SetText = unsafe extern "C" fn(Ptr, *const c_char);
pub type Callback = unsafe extern "C" fn(Ptr, *const c_void);

pub struct Api {
    // Must outlive all copied function pointers, engines and callbacks.
    _library: Library,
    settings_new:
        unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, *const c_char) -> Ptr,
    settings_delete: Delete,
    settings_threads: SetInt,
    settings_context: SetInt,
    engine_new: unsafe extern "C" fn(Ptr) -> Ptr,
    engine_delete: Delete,
    session_new: New,
    session_delete: Delete,
    session_output: SetInt,
    sampler_new: unsafe extern "C" fn(c_int) -> Ptr,
    sampler_delete: Delete,
    sampler_top_p: unsafe extern "C" fn(Ptr, f32),
    sampler_top_k: SetInt,
    sampler_temperature: unsafe extern "C" fn(Ptr, f32),
    session_sampler: SetPtr,
    config_new: New,
    config_delete: Delete,
    config_session: SetPtr,
    config_system: SetText,
    config_messages: SetText,
    thinking_new: New,
    thinking_delete: Delete,
    thinking_enable: unsafe extern "C" fn(Ptr, bool),
    config_thinking: SetPtr,
    conversation_new: unsafe extern "C" fn(Ptr, Ptr) -> Ptr,
    conversation_delete: Delete,
    conversation_stream:
        unsafe extern "C" fn(Ptr, *const c_char, *const c_char, Ptr, Callback, Ptr) -> c_int,
    conversation_cancel: Delete,
    conversation_preface: unsafe extern "C" fn(Ptr) -> *const c_char,
    chunk_text: unsafe extern "C" fn(*const c_void) -> *const c_char,
    chunk_error: unsafe extern "C" fn(*const c_void) -> *const c_char,
    chunk_final: unsafe extern "C" fn(*const c_void) -> bool,
}

macro_rules! symbol {
    ($lib:expr, $name:literal) => {
        *$lib
            .get(concat!($name, "\0").as_bytes())
            .map_err(|e| format!("Missing LiteRT 0.18.0 export {}: {e}", $name))?
    };
}

impl Api {
    pub fn load(path: &Path) -> Result<Arc<Self>, String> {
        // Only an explicit canonical local path is accepted; no PATH search.
        let path = path
            .canonicalize()
            .map_err(|e| format!("Runtime path: {e}"))?;
        unsafe {
            let lib = Library::new(path).map_err(|e| format!("Load runtime: {e}"))?;
            let api = Self {
                settings_new: symbol!(lib, "litert_lm_engine_settings_create"),
                settings_delete: symbol!(lib, "litert_lm_engine_settings_delete"),
                settings_threads: symbol!(lib, "litert_lm_engine_settings_set_num_threads"),
                settings_context: symbol!(lib, "litert_lm_engine_settings_set_max_num_tokens"),
                engine_new: symbol!(lib, "litert_lm_engine_create"),
                engine_delete: symbol!(lib, "litert_lm_engine_delete"),
                session_new: symbol!(lib, "litert_lm_session_config_create"),
                session_delete: symbol!(lib, "litert_lm_session_config_delete"),
                session_output: symbol!(lib, "litert_lm_session_config_set_max_output_tokens"),
                sampler_new: symbol!(lib, "litert_lm_sampler_params_create"),
                sampler_delete: symbol!(lib, "litert_lm_sampler_params_delete"),
                sampler_top_p: symbol!(lib, "litert_lm_sampler_params_set_top_p"),
                sampler_top_k: symbol!(lib, "litert_lm_sampler_params_set_top_k"),
                sampler_temperature: symbol!(lib, "litert_lm_sampler_params_set_temperature"),
                session_sampler: symbol!(lib, "litert_lm_session_config_set_sampler_params"),
                config_new: symbol!(lib, "litert_lm_conversation_config_create"),
                config_delete: symbol!(lib, "litert_lm_conversation_config_delete"),
                config_session: symbol!(lib, "litert_lm_conversation_config_set_session_config"),
                config_system: symbol!(lib, "litert_lm_conversation_config_set_system_message"),
                config_messages: symbol!(lib, "litert_lm_conversation_config_set_messages"),
                thinking_new: symbol!(lib, "litert_lm_thinking_config_create"),
                thinking_delete: symbol!(lib, "litert_lm_thinking_config_delete"),
                thinking_enable: symbol!(lib, "litert_lm_thinking_config_set_enable_thinking"),
                config_thinking: symbol!(lib, "litert_lm_conversation_config_set_thinking_config"),
                conversation_new: symbol!(lib, "litert_lm_conversation_create"),
                conversation_delete: symbol!(lib, "litert_lm_conversation_delete"),
                conversation_stream: symbol!(lib, "litert_lm_conversation_send_message_stream"),
                conversation_cancel: symbol!(lib, "litert_lm_conversation_cancel_process"),
                conversation_preface: symbol!(
                    lib,
                    "litert_lm_conversation_render_preface_to_string"
                ),
                chunk_text: symbol!(lib, "litert_lm_stream_chunk_get_text"),
                chunk_error: symbol!(lib, "litert_lm_stream_chunk_get_error"),
                chunk_final: symbol!(lib, "litert_lm_stream_chunk_is_final"),
                _library: lib,
            };
            Ok(Arc::new(api))
        }
    }

    pub unsafe fn chunk(&self, raw: *const c_void) -> (String, Option<String>, bool) {
        let copy = |p: *const c_char| {
            if p.is_null() {
                None
            } else {
                Some(CStr::from_ptr(p).to_string_lossy().into_owned())
            }
        };
        (
            copy((self.chunk_text)(raw)).unwrap_or_default(),
            copy((self.chunk_error)(raw)),
            (self.chunk_final)(raw),
        )
    }
}

pub struct Engine {
    api: Arc<Api>,
    ptr: Ptr,
}
// Only one request may create/use a conversation. Concurrent control only
// calls conversation_cancel, as supported by the upstream asynchronous API.
unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    pub fn create(
        api: Arc<Api>,
        model: &Path,
        backend: &str,
        threads: i32,
        context: i32,
    ) -> Result<Arc<Self>, String> {
        let model = text(model.to_str().ok_or("Model path must be UTF-8")?)?;
        let backend = text(backend)?;
        unsafe {
            let settings = (api.settings_new)(
                model.as_ptr(),
                backend.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if settings.is_null() {
                return Err("LiteRT engine settings failed".into());
            }
            (api.settings_threads)(settings, threads);
            (api.settings_context)(settings, context);
            let ptr = (api.engine_new)(settings);
            (api.settings_delete)(settings);
            if ptr.is_null() {
                return Err("LiteRT engine initialization failed".into());
            }
            Ok(Arc::new(Self { api, ptr }))
        }
    }

    pub fn conversation(
        self: &Arc<Self>,
        system: &str,
        history: &str,
        output: i32,
        temperature: f32,
    ) -> Result<Arc<Conversation>, String> {
        let system = text(system)?;
        let history = text(history)?;
        unsafe {
            let config = (self.api.config_new)();
            if config.is_null() {
                return Err("Conversation config failed".into());
            }
            let session = (self.api.session_new)();
            let thinking = (self.api.thinking_new)();
            if session.is_null() || thinking.is_null() {
                if !session.is_null() {
                    (self.api.session_delete)(session);
                }
                if !thinking.is_null() {
                    (self.api.thinking_delete)(thinking);
                }
                (self.api.config_delete)(config);
                return Err("Session config failed".into());
            }
            (self.api.session_output)(session, output);
            let sampler = (self.api.sampler_new)(if temperature == 0.0 { 3 } else { 2 }); // GREEDY or TOP_P
            if sampler.is_null() {
                (self.api.session_delete)(session);
                (self.api.thinking_delete)(thinking);
                (self.api.config_delete)(config);
                return Err("Sampler config failed".into());
            }
            (self.api.sampler_top_p)(sampler, 0.95);
            (self.api.sampler_top_k)(sampler, if temperature == 0.0 { 1 } else { 40 });
            (self.api.sampler_temperature)(sampler, temperature);
            (self.api.session_sampler)(session, sampler);
            (self.api.sampler_delete)(sampler);
            (self.api.config_session)(config, session);
            (self.api.session_delete)(session); // Setters copy the configs.
            (self.api.thinking_enable)(thinking, false);
            (self.api.config_thinking)(config, thinking);
            (self.api.thinking_delete)(thinking);
            if !system.as_bytes().is_empty() {
                (self.api.config_system)(config, system.as_ptr());
            }
            (self.api.config_messages)(config, history.as_ptr());
            let ptr = (self.api.conversation_new)(self.ptr, config);
            (self.api.config_delete)(config);
            if ptr.is_null() {
                return Err("Conversation initialization failed".into());
            }
            Ok(Arc::new(Conversation {
                engine: self.clone(),
                ptr,
            }))
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { (self.api.engine_delete)(self.ptr) }
    }
}

pub struct Conversation {
    engine: Arc<Engine>,
    ptr: Ptr,
}
unsafe impl Send for Conversation {}
unsafe impl Sync for Conversation {}
impl Conversation {
    pub fn preface(&self) -> Result<String, String> {
        unsafe {
            let raw = (self.engine.api.conversation_preface)(self.ptr);
            if raw.is_null() {
                return Err("LiteRT failed to render conversation preface".into());
            }
            Ok(CStr::from_ptr(raw).to_string_lossy().into_owned())
        }
    }
    pub fn cancel(&self) {
        unsafe { (self.engine.api.conversation_cancel)(self.ptr) }
    }
    pub unsafe fn stream(
        &self,
        message: &str,
        callback: Callback,
        data: Ptr,
    ) -> Result<(), String> {
        let message = text(message)?;
        let context = text("{}")?;
        let status = (self.engine.api.conversation_stream)(
            self.ptr,
            message.as_ptr(),
            context.as_ptr(),
            std::ptr::null_mut(),
            callback,
            data,
        );
        if status != 0 {
            return Err(format!("LiteRT stream start failed ({status})"));
        }
        Ok(())
    }
}
impl Drop for Conversation {
    fn drop(&mut self) {
        unsafe { (self.engine.api.conversation_delete)(self.ptr) }
    }
}
fn text(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| "Embedded NUL is not accepted".into())
}
