// SPDX-License-Identifier: MPL-2.0
//
// End-to-end tests driving a pure-Rust custom backend through the real Prism C
// library. No screen reader or TTS engine is required: we register our own
// backend, freeze it into a registry, and run a `Context` against it. This
// exercises the full FFI round-trip (registry enumeration, backend lifecycle,
// every vtable slot, error mapping, and callbacks) deterministically on every
// platform.
//
// These tests require the native library to be built/linked, which `build.rs`
// does via CMake. They are the reason the CI "native" job exists.

use prism::{BackendFeatures, Context, CustomBackend, Error, RegistryBuilder, Result};

/// A fully-featured in-memory backend that records what it was told to do.
#[derive(Default)]
struct MockBackend {
    initialized: bool,
    spoken: Vec<(String, bool)>,
    output: Vec<(String, bool)>,
    braille: Vec<String>,
    speaking: bool,
    paused: bool,
    volume: f32,
    rate: f32,
    pitch: f32,
    voice: usize,
    voices: Vec<(&'static str, &'static str)>,
}

impl MockBackend {
    fn new() -> Self {
        MockBackend {
            volume: 0.5,
            rate: 1.0,
            pitch: 1.0,
            voices: vec![("Alice", "en-US"), ("Bob", "en-GB"), ("Claire", "fr-FR")],
            ..Default::default()
        }
    }
}

impl CustomBackend for MockBackend {
    fn initialize(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    fn speak(&mut self, text: &str, interrupt: bool) -> Result<()> {
        if text.is_empty() {
            return Err(Error::InvalidParam);
        }
        if interrupt {
            self.spoken.clear();
        }
        self.spoken.push((text.to_owned(), interrupt));
        self.speaking = true;
        Ok(())
    }

    fn speak_to_memory(
        &mut self,
        text: &str,
        sink: &mut dyn FnMut(&[f32], usize, usize),
    ) -> Result<()> {
        // Emit one sample per character at 22.05kHz mono, then a final flush.
        let samples: Vec<f32> = text.bytes().map(|b| b as f32 / 255.0).collect();
        sink(&samples, 1, 22_050);
        sink(&[], 1, 22_050);
        Ok(())
    }

    fn braille(&mut self, text: &str) -> Result<()> {
        self.braille.push(text.to_owned());
        Ok(())
    }

    fn output(&mut self, text: &str, interrupt: bool) -> Result<()> {
        self.output.push((text.to_owned(), interrupt));
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.speaking = false;
        self.paused = false;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        if !self.speaking {
            return Err(Error::NotSpeaking);
        }
        if self.paused {
            return Err(Error::AlreadyPaused);
        }
        self.paused = true;
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if !self.paused {
            return Err(Error::NotPaused);
        }
        self.paused = false;
        Ok(())
    }

    fn is_speaking(&self) -> Result<bool> {
        Ok(self.speaking && !self.paused)
    }

    fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.volume = volume;
        Ok(())
    }
    fn get_volume(&self) -> Result<f32> {
        Ok(self.volume)
    }
    fn set_rate(&mut self, rate: f32) -> Result<()> {
        self.rate = rate;
        Ok(())
    }
    fn get_rate(&self) -> Result<f32> {
        Ok(self.rate)
    }
    fn set_pitch(&mut self, pitch: f32) -> Result<()> {
        self.pitch = pitch;
        Ok(())
    }
    fn get_pitch(&self) -> Result<f32> {
        Ok(self.pitch)
    }

    fn refresh_voices(&mut self) -> Result<()> {
        Ok(())
    }
    fn count_voices(&self) -> Result<usize> {
        Ok(self.voices.len())
    }
    fn voice_name(&self, voice_id: usize) -> Result<String> {
        self.voices
            .get(voice_id)
            .map(|(n, _)| (*n).to_owned())
            .ok_or(Error::VoiceNotFound)
    }
    fn voice_language(&self, voice_id: usize) -> Result<String> {
        self.voices
            .get(voice_id)
            .map(|(_, l)| (*l).to_owned())
            .ok_or(Error::VoiceNotFound)
    }
    fn set_voice(&mut self, voice_id: usize) -> Result<()> {
        if voice_id >= self.voices.len() {
            return Err(Error::RangeOutOfBounds);
        }
        self.voice = voice_id;
        Ok(())
    }
    fn get_voice(&self) -> Result<usize> {
        Ok(self.voice)
    }

    fn channels(&self) -> Result<usize> {
        Ok(1)
    }
    fn sample_rate(&self) -> Result<usize> {
        Ok(22_050)
    }
    fn bit_depth(&self) -> Result<usize> {
        Ok(32)
    }
}

/// The capabilities `MockBackend` actually implements (everything except the
/// reserved `FEATURE_MAX_BIT`).
fn mock_features() -> BackendFeatures {
    BackendFeatures::all().difference(BackendFeatures::FEATURE_MAX_BIT)
}

/// Build a context whose only backend is our mock, registered under `name`.
fn mock_context(name: &str, priority: i32) -> (Context, prism::BackendId) {
    let mut builder = RegistryBuilder::new().expect("builder");
    let id = builder
        .add_backend(name, priority, mock_features(), MockBackend::new)
        .expect("add_backend");
    let registry = builder.freeze().expect("freeze");
    let ctx = Context::builder()
        .registry(registry)
        .build()
        .expect("context");
    (ctx, id)
}

#[test]
fn registry_enumeration() {
    let (ctx, id) = mock_context("mock", 42);
    assert_eq!(ctx.backend_count(), 1);
    assert_eq!(ctx.id_at(0), Some(id));
    assert_eq!(ctx.id_at(1), None);
    assert_eq!(ctx.id_by_name("mock"), Some(id));
    assert_eq!(ctx.id_by_name("nope"), None);
    assert_eq!(ctx.name_of(id).as_deref(), Some("mock"));
    assert_eq!(ctx.priority_of(id), 42);
    assert!(ctx.exists(id));
    assert!(!ctx.exists(prism::BackendId(0xBAD)));
}

#[test]
fn backend_lifecycle_and_features() {
    let (ctx, id) = mock_context("mock", 1);
    let backend = ctx.create(id).expect("create");
    assert_eq!(backend.name().unwrap(), "mock");
    // The advertised capabilities we care about survive the round-trip.
    let features = backend.features();
    assert!(features.contains(BackendFeatures::SUPPORTS_SPEAK));
    assert!(features.contains(BackendFeatures::SUPPORTS_SET_VOLUME));
    assert!(features.contains(BackendFeatures::SUPPORTS_GET_BIT_DEPTH));
}

#[test]
fn speak_output_braille_flow() {
    let (ctx, _id) = mock_context("mock", 1);
    let mut backend = ctx.create_best().expect("create_best");

    backend.speak("hello", false).unwrap();
    backend.output("status", true).unwrap();
    backend.braille("dots").unwrap();

    // Empty text is rejected by the backend as an invalid parameter.
    assert_eq!(backend.speak("", false), Err(Error::InvalidParam));
}

#[test]
fn pause_resume_state_machine() {
    let (ctx, _id) = mock_context("mock", 1);
    let mut backend = ctx.acquire_best().expect("acquire_best");

    // Not speaking yet.
    assert_eq!(backend.pause(), Err(Error::NotSpeaking));
    backend.speak("hi", false).unwrap();
    assert!(backend.is_speaking().unwrap());
    backend.pause().unwrap();
    assert!(!backend.is_speaking().unwrap());
    assert_eq!(backend.pause(), Err(Error::AlreadyPaused));
    backend.resume().unwrap();
    assert_eq!(backend.resume(), Err(Error::NotPaused));
    backend.stop().unwrap();
    assert!(!backend.is_speaking().unwrap());
}

#[test]
fn property_round_trips() {
    let (ctx, _id) = mock_context("mock", 1);
    let mut backend = ctx.create_best().unwrap();

    backend.set_volume(0.25).unwrap();
    assert_eq!(backend.volume().unwrap(), 0.25);
    backend.set_rate(2.5).unwrap();
    assert_eq!(backend.rate().unwrap(), 2.5);
    backend.set_pitch(0.75).unwrap();
    assert_eq!(backend.pitch().unwrap(), 0.75);
}

#[test]
fn voice_enumeration_and_selection() {
    let (ctx, _id) = mock_context("mock", 1);
    let mut backend = ctx.create_best().unwrap();

    backend.refresh_voices().unwrap();
    assert_eq!(backend.voice_count().unwrap(), 3);
    assert_eq!(backend.voice_name(0).unwrap(), "Alice");
    assert_eq!(backend.voice_language(2).unwrap(), "fr-FR");
    assert_eq!(backend.voice_name(9), Err(Error::VoiceNotFound));

    backend.set_voice(1).unwrap();
    assert_eq!(backend.voice().unwrap(), 1);
    assert_eq!(backend.set_voice(9), Err(Error::RangeOutOfBounds));
}

#[test]
fn audio_format_and_speak_to_memory() {
    let (ctx, _id) = mock_context("mock", 1);
    let mut backend = ctx.create_best().unwrap();

    assert_eq!(backend.channels().unwrap(), 1);
    assert_eq!(backend.sample_rate().unwrap(), 22_050);
    assert_eq!(backend.bit_depth().unwrap(), 32);

    let mut total = 0usize;
    let mut chunks = 0usize;
    backend
        .speak_to_memory("abc", |samples, channels, rate| {
            total += samples.len();
            chunks += 1;
            assert_eq!(channels, 1);
            assert_eq!(rate, 22_050);
        })
        .unwrap();
    assert_eq!(total, 3); // one sample per byte of "abc"
    assert_eq!(chunks, 2); // data chunk + flush
}

#[test]
fn best_selection_respects_priority() {
    let mut builder = RegistryBuilder::new().unwrap();
    builder
        .add_backend("low", 1, mock_features(), MockBackend::new)
        .unwrap();
    let high = builder
        .add_backend("high", 100, mock_features(), MockBackend::new)
        .unwrap();
    let ctx = Context::builder()
        .registry(builder.freeze().unwrap())
        .build()
        .unwrap();

    assert_eq!(ctx.backend_count(), 2);
    let best = ctx.create_best().unwrap();
    assert_eq!(ctx.id_by_name("high"), Some(high));
    assert_eq!(best.name().unwrap(), "high");
}

#[test]
fn error_string_is_populated() {
    // The C library provides a message for every error code.
    let msg = prism::error_string(Error::InvalidParam);
    assert!(!msg.is_empty());
}
