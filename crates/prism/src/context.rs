// SPDX-License-Identifier: MPL-2.0
//! The Prism [`Context`]: library lifecycle and backend registry access.

use crate::backend::Backend;
use crate::error::{Error, Result};
use crate::registry::Registry;
use crate::util::owned_string;
use prism_sys as sys;
use prism_types::{BackendId, CONFIG_VERSION};
use std::ffi::CString;

/// A boxed availability-change callback.
type AvailBox = Box<dyn Fn(BackendId, &str, bool) + Send + Sync>;

/// A running Prism instance.
///
/// Construct with [`Context::new`] for defaults, or [`Context::builder`] to
/// supply a custom [`Registry`] and availability-polling options. The context
/// owns the library session and shuts it down on drop.
pub struct Context {
    raw: *mut sys::PrismContext,
    // Kept alive for the lifetime of the context; order of fields does not
    // matter for correctness because `shutdown` runs first in `Drop`.
    _registry: Option<Registry>,
    _availability: Option<Box<AvailBox>>,
}

// The context is used behind `&self`/`&mut self`; Prism's context is safe to
// move between threads.
unsafe impl Send for Context {}

impl Context {
    /// Initialize Prism with default configuration and the built-in registry.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// Start configuring a context.
    pub fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }

    /// Whether automatic power-management of availability polling is supported
    /// on this platform.
    pub fn auto_power_supported() -> bool {
        // SAFETY: no preconditions.
        unsafe { sys::prism_availability_auto_power_supported() }
    }

    /// The number of registered backends.
    pub fn backend_count(&self) -> usize {
        // SAFETY: `raw` is a valid context.
        unsafe { sys::prism_registry_count(self.raw) }
    }

    /// The backend id at `index`, or `None` if out of range.
    pub fn id_at(&self, index: usize) -> Option<BackendId> {
        // SAFETY: `raw` is a valid context.
        let id = unsafe { sys::prism_registry_id_at(self.raw, index) };
        (id != 0).then_some(BackendId(id))
    }

    /// The backend id registered under `name`, if any.
    pub fn id_by_name(&self, name: &str) -> Option<BackendId> {
        let c = CString::new(name).ok()?;
        // SAFETY: valid context + NUL-terminated name.
        let id = unsafe { sys::prism_registry_id(self.raw, c.as_ptr()) };
        (id != 0).then_some(BackendId(id))
    }

    /// The registry name of `id`, if it exists.
    pub fn name_of(&self, id: BackendId) -> Option<String> {
        // SAFETY: valid context.
        let ptr = unsafe { sys::prism_registry_name(self.raw, id.get()) };
        // SAFETY: `ptr` is null or a static/borrowed C string.
        unsafe { owned_string(ptr) }.ok()
    }

    /// The scheduling priority of `id` (higher wins in `*_best`).
    pub fn priority_of(&self, id: BackendId) -> i32 {
        // SAFETY: valid context.
        unsafe { sys::prism_registry_priority(self.raw, id.get()) as i32 }
    }

    /// Whether a backend with `id` is registered.
    pub fn exists(&self, id: BackendId) -> bool {
        // SAFETY: valid context.
        unsafe { sys::prism_registry_exists(self.raw, id.get()) }
    }

    /// Create a fresh instance of backend `id`.
    pub fn create(&self, id: BackendId) -> Result<Backend> {
        // SAFETY: valid context.
        let raw = unsafe { sys::prism_registry_create(self.raw, id.get()) };
        // SAFETY: `raw` is a freshly-created owned backend or null.
        unsafe { Backend::from_owned(raw) }
    }

    /// Create a fresh instance of the highest-priority available backend.
    pub fn create_best(&self) -> Result<Backend> {
        // SAFETY: valid context.
        let raw = unsafe { sys::prism_registry_create_best(self.raw) };
        unsafe { Backend::from_owned(raw) }
    }

    /// Acquire a shared instance of backend `id`.
    pub fn acquire(&self, id: BackendId) -> Result<Backend> {
        // SAFETY: valid context.
        let raw = unsafe { sys::prism_registry_acquire(self.raw, id.get()) };
        unsafe { Backend::from_owned(raw) }
    }

    /// Acquire a shared instance of the highest-priority available backend.
    pub fn acquire_best(&self) -> Result<Backend> {
        // SAFETY: valid context.
        let raw = unsafe { sys::prism_registry_acquire_best(self.raw) };
        unsafe { Backend::from_owned(raw) }
    }

    /// Pause the background availability-polling thread.
    pub fn pause_availability_polling(&self) {
        // SAFETY: valid context.
        unsafe { sys::prism_availability_poll_pause(self.raw) };
    }

    /// Resume the background availability-polling thread.
    pub fn resume_availability_polling(&self) {
        // SAFETY: valid context.
        unsafe { sys::prism_availability_poll_resume(self.raw) };
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: `raw` is a valid context; shutdown must precede releasing the
        // registry and dropping the availability callback.
        unsafe { sys::prism_shutdown(self.raw) };
    }
}

/// Builder for a [`Context`].
#[derive(Default)]
pub struct ContextBuilder {
    registry: Option<Registry>,
    on_availability: Option<AvailBox>,
    poll_interval_ms: u32,
    debounce_samples: u32,
    backoff_max_ms: u32,
    auto_power_manage: Option<bool>,
}

impl ContextBuilder {
    /// Use a custom [`Registry`] instead of the built-in backends.
    pub fn registry(mut self, registry: Registry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Register a callback invoked when a backend's availability changes.
    ///
    /// The callback runs on Prism's internal polling thread.
    pub fn on_availability<F>(mut self, callback: F) -> Self
    where
        F: Fn(BackendId, &str, bool) + Send + Sync + 'static,
    {
        self.on_availability = Some(Box::new(callback));
        self
    }

    /// Availability poll interval in milliseconds (`0` = library default).
    pub fn poll_interval_ms(mut self, ms: u32) -> Self {
        self.poll_interval_ms = ms;
        self
    }

    /// Number of consistent samples required before reporting a change.
    pub fn debounce_samples(mut self, n: u32) -> Self {
        self.debounce_samples = n;
        self
    }

    /// Maximum polling backoff in milliseconds.
    pub fn backoff_max_ms(mut self, ms: u32) -> Self {
        self.backoff_max_ms = ms;
        self
    }

    /// Whether Prism should auto-manage polling on power events.
    pub fn auto_power_manage(mut self, enabled: bool) -> Self {
        self.auto_power_manage = Some(enabled);
        self
    }

    /// Initialize the context.
    pub fn build(self) -> Result<Context> {
        // Start from library defaults so unset fields are correct.
        let mut cfg = unsafe { sys::prism_config_init() };
        cfg.version = CONFIG_VERSION;

        if let Some(reg) = &self.registry {
            cfg.registry = reg.as_ptr();
        }

        // Keep the boxed callback alive for the whole context and hand its
        // stable heap address to the C side as userdata.
        let availability = self.on_availability.map(|cb| {
            let holder: Box<AvailBox> = Box::new(cb);
            let ptr = &*holder as *const AvailBox as *mut libc::c_void;
            cfg.availability_callback = Some(availability_trampoline);
            cfg.availability_userdata = ptr;
            holder
        });

        cfg.availability_poll_interval_ms = self.poll_interval_ms;
        cfg.availability_debounce_samples = self.debounce_samples;
        cfg.availability_backoff_max_ms = self.backoff_max_ms;
        if let Some(v) = self.auto_power_manage {
            cfg.availability_auto_power_manage = v;
        }

        // SAFETY: `cfg` is fully initialized and valid for the duration of the
        // call; `prism_init` copies what it needs.
        let raw = unsafe { sys::prism_init(&mut cfg) };
        if raw.is_null() {
            return Err(Error::NotInitialized);
        }

        Ok(Context {
            raw,
            _registry: self.registry,
            _availability: availability,
        })
    }
}

extern "C" fn availability_trampoline(
    userdata: *mut libc::c_void,
    backend: sys::PrismBackendId,
    name: *const libc::c_char,
    available: bool,
) {
    if userdata.is_null() {
        return;
    }
    // SAFETY: `userdata` is the stable address of the `AvailBox` stored in the
    // owning context, which outlives all callbacks (shutdown joins the thread).
    let cb = unsafe { &*(userdata as *const AvailBox) };
    let name = if name.is_null() {
        ""
    } else {
        // SAFETY: `name` is a valid NUL-terminated string for this call.
        unsafe { core::ffi::CStr::from_ptr(name) }
            .to_str()
            .unwrap_or("")
    };
    cb(BackendId(backend), name, available);
}
