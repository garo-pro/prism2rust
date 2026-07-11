// SPDX-License-Identifier: MPL-2.0
//! Custom backends and registries.
//!
//! A [`RegistryBuilder`] lets you register pure-Rust [`CustomBackend`]s and
//! freeze them into a [`Registry`] that a [`Context`](crate::Context) can use
//! instead of the built-in platform backends. This is the mechanism the
//! integration tests use to exercise the whole API deterministically and
//! cross-platform, with no real screen reader or TTS engine present.

use crate::error::{Error, Result};
use prism_sys as sys;
use prism_types::BackendId;
use std::ffi::{CStr, CString};

/// A pure-Rust backend implementation.
///
/// Every method has a default that reports [`Error::NotImplemented`] (or a
/// sensible no-op), so implementors override only what they support. The
/// capabilities you advertise to [`RegistryBuilder::add_backend`] should match
/// the methods you actually implement.
///
/// Instances are created on demand by the factory passed to `add_backend`; each
/// method is called by the C library with exclusive access to `self`.
#[allow(unused_variables)]
pub trait CustomBackend: Send + 'static {
    /// Whether this backend can run in the current environment.
    fn is_supported(&self) -> bool {
        true
    }
    /// One-time initialization.
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }
    /// Speak `text`.
    fn speak(&mut self, text: &str, interrupt: bool) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Synthesize `text` to PCM, pushing buffers into `sink`.
    fn speak_to_memory(
        &mut self,
        text: &str,
        sink: &mut dyn FnMut(&[f32], usize, usize),
    ) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Send `text` to a braille display.
    fn braille(&mut self, text: &str) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Output `text` via the most appropriate channel.
    fn output(&mut self, text: &str, interrupt: bool) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Stop speech.
    fn stop(&mut self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Pause speech.
    fn pause(&mut self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Resume speech.
    fn resume(&mut self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Whether speech is in progress.
    fn is_speaking(&self) -> Result<bool> {
        Err(Error::NotImplemented)
    }
    /// Set volume.
    fn set_volume(&mut self, volume: f32) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Get volume.
    fn get_volume(&self) -> Result<f32> {
        Err(Error::NotImplemented)
    }
    /// Set rate.
    fn set_rate(&mut self, rate: f32) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Get rate.
    fn get_rate(&self) -> Result<f32> {
        Err(Error::NotImplemented)
    }
    /// Set pitch.
    fn set_pitch(&mut self, pitch: f32) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Get pitch.
    fn get_pitch(&self) -> Result<f32> {
        Err(Error::NotImplemented)
    }
    /// Refresh the voice list.
    fn refresh_voices(&mut self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// Number of voices.
    fn count_voices(&self) -> Result<usize> {
        Err(Error::NotImplemented)
    }
    /// Name of the voice at `voice_id`.
    fn voice_name(&self, voice_id: usize) -> Result<String> {
        Err(Error::NotImplemented)
    }
    /// Language of the voice at `voice_id`.
    fn voice_language(&self, voice_id: usize) -> Result<String> {
        Err(Error::NotImplemented)
    }
    /// Select the active voice.
    fn set_voice(&mut self, voice_id: usize) -> Result<()> {
        Err(Error::NotImplemented)
    }
    /// The active voice index.
    fn get_voice(&self) -> Result<usize> {
        Err(Error::NotImplemented)
    }
    /// Output channel count.
    fn channels(&self) -> Result<usize> {
        Err(Error::NotImplemented)
    }
    /// Output sample rate (Hz).
    fn sample_rate(&self) -> Result<usize> {
        Err(Error::NotImplemented)
    }
    /// Output bit depth.
    fn bit_depth(&self) -> Result<usize> {
        Err(Error::NotImplemented)
    }
}

/// Per-instance state stored behind the C `void *instance` pointer.
struct Instance<B: CustomBackend> {
    backend: B,
    // Storage backing the borrowed `const char*` returned by voice queries.
    // Kept alive at least until the next query on the same instance.
    name_scratch: Option<CString>,
    lang_scratch: Option<CString>,
}

#[inline]
fn to_code(r: Result<()>) -> sys::PrismError {
    match r {
        Ok(()) => sys::PRISM_OK,
        Err(e) => e.code() as sys::PrismError,
    }
}

// --- vtable trampolines (monomorphized per backend type) -------------------

extern "C" fn tramp_create<F, B>(userdata: *mut libc::c_void) -> *mut libc::c_void
where
    F: Fn() -> B + Send + Sync + 'static,
    B: CustomBackend,
{
    // SAFETY: `userdata` is the `*mut F` stored by `add_backend`.
    let factory = unsafe { &*(userdata as *const F) };
    let instance = Box::new(Instance {
        backend: factory(),
        name_scratch: None,
        lang_scratch: None,
    });
    Box::into_raw(instance) as *mut libc::c_void
}

extern "C" fn tramp_destroy<B: CustomBackend>(instance: *mut libc::c_void) {
    if instance.is_null() {
        return;
    }
    // SAFETY: `instance` was produced by `tramp_create` for the same `B`.
    drop(unsafe { Box::from_raw(instance as *mut Instance<B>) });
}

extern "C" fn factory_free<F>(userdata: *mut libc::c_void) {
    if userdata.is_null() {
        return;
    }
    // SAFETY: `userdata` was `Box::into_raw`'d from a `Box<F>`.
    drop(unsafe { Box::from_raw(userdata as *mut F) });
}

/// Borrow the `Instance<B>` behind a raw pointer.
///
/// # Safety
/// `instance` must be a valid `*mut Instance<B>` from `tramp_create::<_, B>`.
unsafe fn inst<'a, B: CustomBackend>(instance: *mut libc::c_void) -> &'a mut Instance<B> {
    &mut *(instance as *mut Instance<B>)
}

extern "C" fn tramp_is_supported<B: CustomBackend>(instance: *mut libc::c_void) -> bool {
    unsafe { inst::<B>(instance) }.backend.is_supported()
}

extern "C" fn tramp_initialize<B: CustomBackend>(instance: *mut libc::c_void) -> sys::PrismError {
    to_code(unsafe { inst::<B>(instance) }.backend.initialize())
}

extern "C" fn tramp_speak<B: CustomBackend>(
    instance: *mut libc::c_void,
    text: *const libc::c_char,
    interrupt: bool,
) -> sys::PrismError {
    match unsafe { borrow_str(text) } {
        Ok(s) => to_code(unsafe { inst::<B>(instance) }.backend.speak(s, interrupt)),
        Err(code) => code,
    }
}

extern "C" fn tramp_speak_to_memory<B: CustomBackend>(
    instance: *mut libc::c_void,
    text: *const libc::c_char,
    callback: sys::PrismAudioCallback,
    callback_userdata: *mut libc::c_void,
) -> sys::PrismError {
    let s = match unsafe { borrow_str(text) } {
        Ok(s) => s,
        Err(code) => return code,
    };
    let mut sink = |samples: &[f32], channels: usize, sample_rate: usize| {
        if let Some(cb) = callback {
            // SAFETY: forwarding the buffer the trait handed us to the C sink.
            unsafe {
                cb(
                    callback_userdata,
                    samples.as_ptr(),
                    samples.len(),
                    channels,
                    sample_rate,
                );
            }
        }
    };
    to_code(
        unsafe { inst::<B>(instance) }
            .backend
            .speak_to_memory(s, &mut sink),
    )
}

extern "C" fn tramp_braille<B: CustomBackend>(
    instance: *mut libc::c_void,
    text: *const libc::c_char,
) -> sys::PrismError {
    match unsafe { borrow_str(text) } {
        Ok(s) => to_code(unsafe { inst::<B>(instance) }.backend.braille(s)),
        Err(code) => code,
    }
}

extern "C" fn tramp_output<B: CustomBackend>(
    instance: *mut libc::c_void,
    text: *const libc::c_char,
    interrupt: bool,
) -> sys::PrismError {
    match unsafe { borrow_str(text) } {
        Ok(s) => to_code(unsafe { inst::<B>(instance) }.backend.output(s, interrupt)),
        Err(code) => code,
    }
}

extern "C" fn tramp_stop<B: CustomBackend>(instance: *mut libc::c_void) -> sys::PrismError {
    to_code(unsafe { inst::<B>(instance) }.backend.stop())
}
extern "C" fn tramp_pause<B: CustomBackend>(instance: *mut libc::c_void) -> sys::PrismError {
    to_code(unsafe { inst::<B>(instance) }.backend.pause())
}
extern "C" fn tramp_resume<B: CustomBackend>(instance: *mut libc::c_void) -> sys::PrismError {
    to_code(unsafe { inst::<B>(instance) }.backend.resume())
}

extern "C" fn tramp_is_speaking<B: CustomBackend>(
    instance: *mut libc::c_void,
    out: *mut bool,
) -> sys::PrismError {
    match unsafe { inst::<B>(instance) }.backend.is_speaking() {
        Ok(v) => {
            unsafe { *out = v };
            sys::PRISM_OK
        }
        Err(e) => e.code() as sys::PrismError,
    }
}

// Generates a set/get f32 trampoline pair for a property.
macro_rules! prop_f32 {
    ($set:ident, $get:ident, $tset:ident, $tget:ident) => {
        extern "C" fn $tset<B: CustomBackend>(
            instance: *mut libc::c_void,
            value: f32,
        ) -> sys::PrismError {
            to_code(unsafe { inst::<B>(instance) }.backend.$set(value))
        }
        extern "C" fn $tget<B: CustomBackend>(
            instance: *mut libc::c_void,
            out: *mut f32,
        ) -> sys::PrismError {
            match unsafe { inst::<B>(instance) }.backend.$get() {
                Ok(v) => {
                    unsafe { *out = v };
                    sys::PRISM_OK
                }
                Err(e) => e.code() as sys::PrismError,
            }
        }
    };
}
prop_f32!(set_volume, get_volume, tramp_set_volume, tramp_get_volume);
prop_f32!(set_rate, get_rate, tramp_set_rate, tramp_get_rate);
prop_f32!(set_pitch, get_pitch, tramp_set_pitch, tramp_get_pitch);

extern "C" fn tramp_refresh_voices<B: CustomBackend>(
    instance: *mut libc::c_void,
) -> sys::PrismError {
    to_code(unsafe { inst::<B>(instance) }.backend.refresh_voices())
}

// Generates a `usize` out-param getter trampoline.
macro_rules! getter_usize {
    ($method:ident, $tramp:ident) => {
        extern "C" fn $tramp<B: CustomBackend>(
            instance: *mut libc::c_void,
            out: *mut usize,
        ) -> sys::PrismError {
            match unsafe { inst::<B>(instance) }.backend.$method() {
                Ok(v) => {
                    unsafe { *out = v };
                    sys::PRISM_OK
                }
                Err(e) => e.code() as sys::PrismError,
            }
        }
    };
}
getter_usize!(count_voices, tramp_count_voices);
getter_usize!(get_voice, tramp_get_voice);
getter_usize!(channels, tramp_get_channels);
getter_usize!(sample_rate, tramp_get_sample_rate);
getter_usize!(bit_depth, tramp_get_bit_depth);

extern "C" fn tramp_set_voice<B: CustomBackend>(
    instance: *mut libc::c_void,
    voice_id: usize,
) -> sys::PrismError {
    to_code(unsafe { inst::<B>(instance) }.backend.set_voice(voice_id))
}

extern "C" fn tramp_get_voice_name<B: CustomBackend>(
    instance: *mut libc::c_void,
    voice_id: usize,
    out: *mut *const libc::c_char,
) -> sys::PrismError {
    let i = unsafe { inst::<B>(instance) };
    match i.backend.voice_name(voice_id) {
        Ok(name) => match CString::new(name) {
            Ok(c) => {
                let ptr = c.as_ptr();
                i.name_scratch = Some(c);
                unsafe { *out = ptr };
                sys::PRISM_OK
            }
            Err(_) => Error::InvalidUtf8.code() as sys::PrismError,
        },
        Err(e) => e.code() as sys::PrismError,
    }
}

extern "C" fn tramp_get_voice_language<B: CustomBackend>(
    instance: *mut libc::c_void,
    voice_id: usize,
    out: *mut *const libc::c_char,
) -> sys::PrismError {
    let i = unsafe { inst::<B>(instance) };
    match i.backend.voice_language(voice_id) {
        Ok(lang) => match CString::new(lang) {
            Ok(c) => {
                let ptr = c.as_ptr();
                i.lang_scratch = Some(c);
                unsafe { *out = ptr };
                sys::PRISM_OK
            }
            Err(_) => Error::InvalidUtf8.code() as sys::PrismError,
        },
        Err(e) => e.code() as sys::PrismError,
    }
}

/// Borrow a C string argument as `&str`, mapping problems to error codes.
///
/// # Safety
/// `text` must be null or a valid NUL-terminated string for the call.
unsafe fn borrow_str<'a>(
    text: *const libc::c_char,
) -> core::result::Result<&'a str, sys::PrismError> {
    if text.is_null() {
        return Err(Error::InvalidParam.code() as sys::PrismError);
    }
    CStr::from_ptr(text)
        .to_str()
        .map_err(|_| Error::InvalidUtf8.code() as sys::PrismError)
}

/// Build the fully-populated vtable for a `(factory, backend)` type pair.
fn build_vtable<F, B>() -> sys::PrismBackendVTable
where
    F: Fn() -> B + Send + Sync + 'static,
    B: CustomBackend,
{
    sys::PrismBackendVTable {
        size: core::mem::size_of::<sys::PrismBackendVTable>(),
        create: Some(tramp_create::<F, B>),
        destroy: Some(tramp_destroy::<B>),
        is_supported: Some(tramp_is_supported::<B>),
        initialize: Some(tramp_initialize::<B>),
        speak: Some(tramp_speak::<B>),
        speak_to_memory: Some(tramp_speak_to_memory::<B>),
        braille: Some(tramp_braille::<B>),
        output: Some(tramp_output::<B>),
        stop: Some(tramp_stop::<B>),
        pause: Some(tramp_pause::<B>),
        resume: Some(tramp_resume::<B>),
        is_speaking: Some(tramp_is_speaking::<B>),
        set_volume: Some(tramp_set_volume::<B>),
        get_volume: Some(tramp_get_volume::<B>),
        set_rate: Some(tramp_set_rate::<B>),
        get_rate: Some(tramp_get_rate::<B>),
        set_pitch: Some(tramp_set_pitch::<B>),
        get_pitch: Some(tramp_get_pitch::<B>),
        refresh_voices: Some(tramp_refresh_voices::<B>),
        count_voices: Some(tramp_count_voices::<B>),
        get_voice_name: Some(tramp_get_voice_name::<B>),
        get_voice_language: Some(tramp_get_voice_language::<B>),
        set_voice: Some(tramp_set_voice::<B>),
        get_voice: Some(tramp_get_voice::<B>),
        get_channels: Some(tramp_get_channels::<B>),
        get_sample_rate: Some(tramp_get_sample_rate::<B>),
        get_bit_depth: Some(tramp_get_bit_depth::<B>),
    }
}

/// A frozen, reference-counted set of backends.
///
/// Clone to share ownership (increments the upstream refcount); drop to release.
pub struct Registry {
    raw: *mut sys::PrismRegistry,
}

// The upstream registry is internally reference-counted and thread-safe.
unsafe impl Send for Registry {}
unsafe impl Sync for Registry {}

impl Registry {
    pub(crate) fn as_ptr(&self) -> *mut sys::PrismRegistry {
        self.raw
    }
}

impl Clone for Registry {
    fn clone(&self) -> Self {
        // SAFETY: `raw` is a live registry; retain bumps its refcount.
        let raw = unsafe { sys::prism_registry_retain(self.raw) };
        Registry { raw }
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        // SAFETY: balanced against the retain/freeze that produced `raw`.
        unsafe { sys::prism_registry_release(self.raw) };
    }
}

/// Builds a [`Registry`] from one or more [`CustomBackend`]s.
pub struct RegistryBuilder {
    raw: *mut sys::PrismRegistryBuilder,
}

impl RegistryBuilder {
    /// Create an empty builder.
    pub fn new() -> Result<Self> {
        // SAFETY: constructor with no preconditions.
        let raw = unsafe { sys::prism_registry_builder_new() };
        if raw.is_null() {
            return Err(Error::MemoryFailure);
        }
        Ok(RegistryBuilder { raw })
    }

    /// Register a backend produced by `factory`.
    ///
    /// * `name` — the backend's registry name (used for lookup by name).
    /// * `priority` — higher priority wins in `create_best`/`acquire_best`.
    /// * `features` — the capabilities the backend advertises.
    /// * `factory` — called to construct a fresh instance on demand.
    ///
    /// Returns the [`BackendId`] the library minted for this backend.
    pub fn add_backend<F, B>(
        &mut self,
        name: &str,
        priority: i32,
        features: prism_types::BackendFeatures,
        factory: F,
    ) -> Result<BackendId>
    where
        F: Fn() -> B + Send + Sync + 'static,
        B: CustomBackend,
    {
        let c_name = CString::new(name).map_err(|_| Error::InvalidParam)?;

        // The vtable must outlive the registry. Leak one per registration; it
        // is a small, bounded, one-time allocation keyed by the `(F, B)` types.
        let vtable: &'static sys::PrismBackendVTable = Box::leak(Box::new(build_vtable::<F, B>()));

        // The factory is owned by the registry via `userdata`/`userdata_free`.
        let userdata = Box::into_raw(Box::new(factory)) as *mut libc::c_void;

        let mut out_id: sys::PrismBackendId = 0;
        // SAFETY: all pointers are valid; `userdata_free` matches how `userdata`
        // was allocated; `vtable` is 'static.
        let code = unsafe {
            sys::prism_registry_builder_add_backend(
                self.raw,
                c_name.as_ptr(),
                priority as libc::c_int,
                features.bits(),
                vtable as *const sys::PrismBackendVTable,
                userdata,
                Some(factory_free::<F>),
                &mut out_id,
            )
        };
        if let Err(e) = Error::check(code as i32) {
            // On failure the library did not take ownership of `userdata`.
            drop(unsafe { Box::from_raw(userdata as *mut F) });
            return Err(e);
        }
        Ok(BackendId(out_id))
    }

    /// Consume the builder and produce a frozen [`Registry`].
    pub fn freeze(mut self) -> Result<Registry> {
        // SAFETY: `raw` is a live builder; freeze consumes it.
        let reg = unsafe { sys::prism_registry_freeze(self.raw) };
        // `freeze` consumes the builder; neutralize our Drop.
        self.raw = core::ptr::null_mut();
        if reg.is_null() {
            return Err(Error::Internal);
        }
        Ok(Registry { raw: reg })
    }
}

impl Drop for RegistryBuilder {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            // SAFETY: live builder not yet frozen.
            unsafe { sys::prism_registry_builder_free(self.raw) };
        }
    }
}
