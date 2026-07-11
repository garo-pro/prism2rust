// SPDX-License-Identifier: MPL-2.0
//! Safe wrapper around a Prism backend instance.

use crate::error::{check, Error, Result};
use crate::util::{owned_string, to_cstring};
use prism_sys as sys;
use prism_types::BackendFeatures;

/// A live, owned backend instance obtained from a [`Context`](crate::Context).
///
/// The instance is initialized on construction and freed on drop. All methods
/// map 1:1 onto the C ABI; capability is advertised via [`Backend::features`],
/// and calling an unsupported method returns [`Error::NotImplemented`].
pub struct Backend {
    raw: *mut sys::PrismBackend,
}

// A backend instance is owned exclusively by this handle and its methods take
// `&mut self`; it may be moved to another thread.
unsafe impl Send for Backend {}

impl Backend {
    /// Adopt an owned raw backend pointer (from `create`/`acquire`) and run its
    /// one-time initialization.
    ///
    /// # Safety
    /// `raw` must be a non-null, owned `PrismBackend` that is not aliased.
    pub(crate) unsafe fn from_owned(raw: *mut sys::PrismBackend) -> Result<Self> {
        if raw.is_null() {
            return Err(Error::BackendNotAvailable);
        }
        let backend = Backend { raw };
        // Initialization is idempotent upstream; treat "already initialized" as
        // success, mirroring the reference Python binding.
        match check(sys::prism_backend_initialize(raw)) {
            Ok(()) => Ok(backend),
            Err(Error::AlreadyInitialized) => Ok(backend),
            Err(e) => Err(e),
        }
    }

    /// The backend's human-readable name.
    pub fn name(&self) -> Result<String> {
        // SAFETY: `raw` is a valid owned backend.
        unsafe { owned_string(sys::prism_backend_name(self.raw)) }
    }

    /// The capabilities this backend advertises.
    pub fn features(&self) -> BackendFeatures {
        // SAFETY: `raw` is a valid owned backend.
        let bits = unsafe { sys::prism_backend_get_features(self.raw) };
        BackendFeatures::from_bits_retain(bits)
    }

    /// Speak `text`, optionally interrupting current speech.
    pub fn speak(&mut self, text: &str, interrupt: bool) -> Result<()> {
        let c = to_cstring(text)?;
        // SAFETY: valid backend + NUL-terminated string.
        check(unsafe { sys::prism_backend_speak(self.raw, c.as_ptr(), interrupt) })
    }

    /// Synthesize `text` to PCM samples delivered to `on_audio`.
    ///
    /// `on_audio` receives `(samples, channels, sample_rate)` and is invoked
    /// synchronously, possibly multiple times, before this call returns.
    pub fn speak_to_memory<F>(&mut self, text: &str, mut on_audio: F) -> Result<()>
    where
        F: FnMut(&[f32], usize, usize),
    {
        let c = to_cstring(text)?;

        extern "C" fn trampoline<F: FnMut(&[f32], usize, usize)>(
            userdata: *mut libc::c_void,
            samples: *const f32,
            sample_count: usize,
            channels: usize,
            sample_rate: usize,
        ) {
            // SAFETY: `userdata` is the `&mut F` we passed below; samples is a
            // valid buffer of `sample_count` floats for the call's duration.
            let cb = unsafe { &mut *(userdata as *mut F) };
            let slice = if sample_count == 0 || samples.is_null() {
                &[][..]
            } else {
                unsafe { core::slice::from_raw_parts(samples, sample_count) }
            };
            cb(slice, channels, sample_rate);
        }

        let userdata = &mut on_audio as *mut F as *mut libc::c_void;
        // SAFETY: valid backend + string; the callback and userdata outlive the
        // synchronous call.
        check(unsafe {
            sys::prism_backend_speak_to_memory(
                self.raw,
                c.as_ptr(),
                Some(trampoline::<F>),
                userdata,
            )
        })
    }

    /// Send `text` to a connected braille display.
    pub fn braille(&mut self, text: &str) -> Result<()> {
        let c = to_cstring(text)?;
        check(unsafe { sys::prism_backend_braille(self.raw, c.as_ptr()) })
    }

    /// Output `text` via the backend's most appropriate channel.
    pub fn output(&mut self, text: &str, interrupt: bool) -> Result<()> {
        let c = to_cstring(text)?;
        check(unsafe { sys::prism_backend_output(self.raw, c.as_ptr(), interrupt) })
    }

    /// Stop any in-progress speech.
    pub fn stop(&mut self) -> Result<()> {
        check(unsafe { sys::prism_backend_stop(self.raw) })
    }

    /// Pause speech.
    pub fn pause(&mut self) -> Result<()> {
        check(unsafe { sys::prism_backend_pause(self.raw) })
    }

    /// Resume paused speech.
    pub fn resume(&mut self) -> Result<()> {
        check(unsafe { sys::prism_backend_resume(self.raw) })
    }

    /// Whether the backend is currently speaking.
    pub fn is_speaking(&self) -> Result<bool> {
        let mut out = false;
        check(unsafe { sys::prism_backend_is_speaking(self.raw, &mut out) })?;
        Ok(out)
    }

    /// Set output volume, normalized to `0.0..=1.0`.
    ///
    /// Values outside that range (or non-finite) return
    /// [`Error::RangeOutOfBounds`] without reaching the backend.
    pub fn set_volume(&mut self, volume: f32) -> Result<()> {
        check(unsafe { sys::prism_backend_set_volume(self.raw, volume) })
    }

    /// Get output volume.
    pub fn volume(&self) -> Result<f32> {
        let mut out = 0.0;
        check(unsafe { sys::prism_backend_get_volume(self.raw, &mut out) })?;
        Ok(out)
    }

    /// Set speech rate, normalized to `0.0..=1.0`.
    ///
    /// Values outside that range (or non-finite) return
    /// [`Error::RangeOutOfBounds`] without reaching the backend.
    pub fn set_rate(&mut self, rate: f32) -> Result<()> {
        check(unsafe { sys::prism_backend_set_rate(self.raw, rate) })
    }

    /// Get speech rate.
    pub fn rate(&self) -> Result<f32> {
        let mut out = 0.0;
        check(unsafe { sys::prism_backend_get_rate(self.raw, &mut out) })?;
        Ok(out)
    }

    /// Set speech pitch, normalized to `0.0..=1.0`.
    ///
    /// Values outside that range (or non-finite) return
    /// [`Error::RangeOutOfBounds`] without reaching the backend.
    pub fn set_pitch(&mut self, pitch: f32) -> Result<()> {
        check(unsafe { sys::prism_backend_set_pitch(self.raw, pitch) })
    }

    /// Get speech pitch.
    pub fn pitch(&self) -> Result<f32> {
        let mut out = 0.0;
        check(unsafe { sys::prism_backend_get_pitch(self.raw, &mut out) })?;
        Ok(out)
    }

    /// Refresh the backend's list of available voices.
    pub fn refresh_voices(&mut self) -> Result<()> {
        check(unsafe { sys::prism_backend_refresh_voices(self.raw) })
    }

    /// The number of voices currently available.
    pub fn voice_count(&self) -> Result<usize> {
        let mut out = 0;
        check(unsafe { sys::prism_backend_count_voices(self.raw, &mut out) })?;
        Ok(out)
    }

    /// The display name of the voice at `voice_id`.
    pub fn voice_name(&self, voice_id: usize) -> Result<String> {
        let mut ptr: *const libc::c_char = core::ptr::null();
        check(unsafe { sys::prism_backend_get_voice_name(self.raw, voice_id, &mut ptr) })?;
        unsafe { owned_string(ptr) }
    }

    /// The language tag of the voice at `voice_id`.
    pub fn voice_language(&self, voice_id: usize) -> Result<String> {
        let mut ptr: *const libc::c_char = core::ptr::null();
        check(unsafe { sys::prism_backend_get_voice_language(self.raw, voice_id, &mut ptr) })?;
        unsafe { owned_string(ptr) }
    }

    /// Select the active voice by index.
    pub fn set_voice(&mut self, voice_id: usize) -> Result<()> {
        check(unsafe { sys::prism_backend_set_voice(self.raw, voice_id) })
    }

    /// The index of the active voice.
    pub fn voice(&self) -> Result<usize> {
        let mut out = 0;
        check(unsafe { sys::prism_backend_get_voice(self.raw, &mut out) })?;
        Ok(out)
    }

    /// Number of audio channels for `speak_to_memory` output.
    pub fn channels(&self) -> Result<usize> {
        let mut out = 0;
        check(unsafe { sys::prism_backend_get_channels(self.raw, &mut out) })?;
        Ok(out)
    }

    /// Sample rate (Hz) for `speak_to_memory` output.
    pub fn sample_rate(&self) -> Result<usize> {
        let mut out = 0;
        check(unsafe { sys::prism_backend_get_sample_rate(self.raw, &mut out) })?;
        Ok(out)
    }

    /// Bit depth for `speak_to_memory` output.
    pub fn bit_depth(&self) -> Result<usize> {
        let mut out = 0;
        check(unsafe { sys::prism_backend_get_bit_depth(self.raw, &mut out) })?;
        Ok(out)
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        // SAFETY: `raw` is an owned backend created via create/acquire and not
        // freed elsewhere.
        unsafe { sys::prism_backend_free(self.raw) };
    }
}
