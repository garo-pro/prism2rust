// SPDX-License-Identifier: MPL-2.0
//
// Portable demo: implement a pure-Rust backend, register it, and drive the full
// safe API surface against it through the real Prism library — with no screen
// reader or TTS engine present. This runs identically on every platform, which
// is exactly why the test suite uses the same technique.
//
// Run with:  cargo run -p prism --example custom_backend

use prism::{BackendFeatures, Context, CustomBackend, RegistryBuilder, Result};

/// A backend that "renders" speech by printing it and synthesizing trivial PCM.
#[derive(Default)]
struct DemoBackend {
    volume: f32,
    rate: f32,
    pitch: f32,
    voice: usize,
    speaking: bool,
    paused: bool,
}

const VOICES: [(&str, &str); 2] = [("Nova", "en-US"), ("Lyra", "en-GB")];

impl CustomBackend for DemoBackend {
    fn initialize(&mut self) -> Result<()> {
        self.volume = 0.5;
        self.rate = 0.5;
        self.pitch = 0.5;
        println!("[demo] initialized");
        Ok(())
    }

    fn speak(&mut self, text: &str, interrupt: bool) -> Result<()> {
        self.speaking = true;
        println!("[demo] speak(interrupt={interrupt}): {text:?}");
        Ok(())
    }

    fn speak_to_memory(
        &mut self,
        text: &str,
        sink: &mut dyn FnMut(&[f32], usize, usize),
    ) -> Result<()> {
        // One f32 sample per byte, mono @ 24kHz, then a zero-length flush.
        let pcm: Vec<f32> = text.bytes().map(|b| b as f32 / 255.0).collect();
        sink(&pcm, 1, 24_000);
        sink(&[], 1, 24_000);
        Ok(())
    }

    fn braille(&mut self, text: &str) -> Result<()> {
        println!("[demo] braille: {text:?}");
        Ok(())
    }

    fn output(&mut self, text: &str, interrupt: bool) -> Result<()> {
        println!("[demo] output(interrupt={interrupt}): {text:?}");
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.speaking = false;
        self.paused = false;
        println!("[demo] stop");
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.paused = true;
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        self.paused = false;
        Ok(())
    }
    fn is_speaking(&self) -> Result<bool> {
        Ok(self.speaking && !self.paused)
    }

    fn set_volume(&mut self, v: f32) -> Result<()> {
        self.volume = v;
        Ok(())
    }
    fn get_volume(&self) -> Result<f32> {
        Ok(self.volume)
    }
    fn set_rate(&mut self, v: f32) -> Result<()> {
        self.rate = v;
        Ok(())
    }
    fn get_rate(&self) -> Result<f32> {
        Ok(self.rate)
    }
    fn set_pitch(&mut self, v: f32) -> Result<()> {
        self.pitch = v;
        Ok(())
    }
    fn get_pitch(&self) -> Result<f32> {
        Ok(self.pitch)
    }

    fn count_voices(&self) -> Result<usize> {
        Ok(VOICES.len())
    }
    fn voice_name(&self, id: usize) -> Result<String> {
        Ok(VOICES[id].0.to_owned())
    }
    fn voice_language(&self, id: usize) -> Result<String> {
        Ok(VOICES[id].1.to_owned())
    }
    fn set_voice(&mut self, id: usize) -> Result<()> {
        self.voice = id;
        Ok(())
    }
    fn get_voice(&self) -> Result<usize> {
        Ok(self.voice)
    }

    fn channels(&self) -> Result<usize> {
        Ok(1)
    }
    fn sample_rate(&self) -> Result<usize> {
        Ok(24_000)
    }
    fn bit_depth(&self) -> Result<usize> {
        Ok(32)
    }
}

fn main() -> Result<()> {
    // Register our backend. Note the registry is seeded with the platform's
    // built-in backends too, so we keep the id to address ours specifically.
    let mut builder = RegistryBuilder::new()?;
    let id = builder.add_backend(
        "demo",
        200,
        BackendFeatures::all().difference(BackendFeatures::FEATURE_MAX_BIT),
        DemoBackend::default,
    )?;
    let ctx = Context::builder().registry(builder.freeze()?).build()?;

    println!(
        "Registered 'demo' as {id} (context has {} backend(s) total)\n",
        ctx.backend_count()
    );

    let mut backend = ctx.create(id)?;
    println!("Created backend: {}", backend.name()?);

    // Speech + messaging.
    backend.speak("Hello from a custom Rust backend!", false)?;
    backend.output("status line", true)?;

    // Properties (Prism normalizes these to 0.0..=1.0).
    backend.set_volume(0.9)?;
    backend.set_rate(0.3)?;
    backend.set_pitch(0.7)?;
    println!(
        "volume={:.1} rate={:.1} pitch={:.1}",
        backend.volume()?,
        backend.rate()?,
        backend.pitch()?
    );

    // Voices.
    println!("voices: {}", backend.voice_count()?);
    for v in 0..backend.voice_count()? {
        println!(
            "  {} [{}]",
            backend.voice_name(v)?,
            backend.voice_language(v)?
        );
    }
    backend.set_voice(1)?;
    println!("selected voice index: {}", backend.voice()?);

    // Pause/resume state.
    backend.speak("counting", false)?;
    backend.pause()?;
    println!("is_speaking while paused: {}", backend.is_speaking()?);
    backend.resume()?;
    backend.stop()?;

    // Synthesis to memory.
    let mut samples = 0usize;
    backend.speak_to_memory("abcdef", |buf, ch, rate| {
        samples += buf.len();
        if !buf.is_empty() {
            println!("received {} samples ({ch}ch @ {rate}Hz)", buf.len());
        }
    })?;
    println!("total synthesized samples: {samples}");

    // Audio format.
    println!(
        "format: {}ch @ {}Hz, {}-bit",
        backend.channels()?,
        backend.sample_rate()?,
        backend.bit_depth()?
    );

    println!("\nDone.");
    Ok(())
}
