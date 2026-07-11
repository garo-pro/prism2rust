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

// A registry built from `RegistryBuilder` is seeded with the platform's
// built-in backends, so our mock is *added to* them. Tests therefore target the
// mock explicitly by the id returned from `add_backend` (via `Context::create`)
// rather than assuming it is the only/best backend — `create_best` may pick an
// available built-in on the host.

/// Build a context that includes our uniquely-named mock backend.
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
    let (ctx, id) = mock_context("mock_enum", 42);
    // Our backend coexists with the built-in catalog.
    assert!(ctx.backend_count() >= 1);
    assert!(ctx.exists(id));
    assert_eq!(ctx.id_by_name("mock_enum"), Some(id));
    assert_eq!(ctx.name_of(id).as_deref(), Some("mock_enum"));
    assert_eq!(ctx.priority_of(id), 42);
    assert_eq!(ctx.id_by_name("definitely-not-a-backend"), None);
    assert!(!ctx.exists(prism::BackendId(0xBAD)));
    // The id is discoverable by index enumeration.
    let found = (0..ctx.backend_count()).any(|i| ctx.id_at(i) == Some(id));
    assert!(found, "registered backend not found via id_at");
    assert_eq!(ctx.id_at(ctx.backend_count()), None); // out of range
}

#[test]
fn backend_lifecycle_and_features() {
    let (ctx, id) = mock_context("mock_life", 1);
    let backend = ctx.create(id).expect("create");
    assert_eq!(backend.name().unwrap(), "mock_life");
    // The advertised capabilities we care about survive the round-trip.
    let features = backend.features();
    assert!(features.contains(BackendFeatures::SUPPORTS_SPEAK));
    assert!(features.contains(BackendFeatures::SUPPORTS_SET_VOLUME));
    assert!(features.contains(BackendFeatures::SUPPORTS_GET_BIT_DEPTH));
}

#[test]
fn speak_output_braille_flow() {
    let (ctx, id) = mock_context("mock_speak", 1);
    let mut backend = ctx.create(id).expect("create");

    backend.speak("hello", false).unwrap();
    backend.output("status", true).unwrap();
    backend.braille("dots").unwrap();

    // Empty text is valid UTF-8, so it reaches the backend, which rejects it.
    assert_eq!(backend.speak("", false), Err(Error::InvalidParam));
}

#[test]
fn pause_resume_state_machine() {
    let (ctx, id) = mock_context("mock_pause", 1);
    let mut backend = ctx.acquire(id).expect("acquire");

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
    let (ctx, id) = mock_context("mock_props", 1);
    let mut backend = ctx.create(id).unwrap();

    // Prism normalizes volume/rate/pitch to 0.0..=1.0.
    backend.set_volume(0.25).unwrap();
    assert_eq!(backend.volume().unwrap(), 0.25);
    backend.set_rate(0.5).unwrap();
    assert_eq!(backend.rate().unwrap(), 0.5);
    backend.set_pitch(0.75).unwrap();
    assert_eq!(backend.pitch().unwrap(), 0.75);

    // Out-of-range values are rejected by the library before reaching us.
    assert_eq!(backend.set_volume(1.5), Err(Error::RangeOutOfBounds));
    assert_eq!(backend.set_rate(-0.1), Err(Error::RangeOutOfBounds));
    assert_eq!(backend.set_pitch(f32::NAN), Err(Error::RangeOutOfBounds));
}

#[test]
fn voice_enumeration_and_selection() {
    let (ctx, id) = mock_context("mock_voices", 1);
    let mut backend = ctx.create(id).unwrap();

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
    let (ctx, id) = mock_context("mock_audio", 1);
    let mut backend = ctx.create(id).unwrap();

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
fn priorities_are_recorded_and_backends_addressable() {
    let mut builder = RegistryBuilder::new().unwrap();
    let low = builder
        .add_backend("mock_low", 1, mock_features(), MockBackend::new)
        .unwrap();
    let high = builder
        .add_backend("mock_high", 100, mock_features(), MockBackend::new)
        .unwrap();
    let ctx = Context::builder()
        .registry(builder.freeze().unwrap())
        .build()
        .unwrap();

    assert_ne!(low, high);
    assert_eq!(ctx.priority_of(low), 1);
    assert_eq!(ctx.priority_of(high), 100);
    // Each is individually addressable by its id.
    assert_eq!(ctx.create(low).unwrap().name().unwrap(), "mock_low");
    assert_eq!(ctx.create(high).unwrap().name().unwrap(), "mock_high");

    // NOTE: we deliberately do NOT call `create_best`/`acquire_best` here.
    // Because the registry is seeded with the platform's built-in backends,
    // `*_best` can select a *real* backend whose construction is environment
    // dependent (e.g. AVSpeech on macOS blocks when built off the main thread).
    // Those methods are thin wrappers over the same path as `create`, exercised
    // above; the `speak` example uses `acquire_best` for real interactive use.
}

#[test]
fn error_string_is_populated() {
    // The C library provides a message for every error code.
    let msg = prism::error_string(Error::InvalidParam);
    assert!(!msg.is_empty());
}
