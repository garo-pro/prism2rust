// SPDX-License-Identifier: MPL-2.0
//
// Real-backend demo: initialize Prism, enumerate the available backends, pick
// the best one, and speak — adjusting properties only when the chosen backend
// advertises support for them.
//
// This talks to the actual platform screen reader / TTS engine, so it produces
// audible output and requires a backend to be present. On a machine with none
// available (e.g. a headless CI runner) it degrades gracefully and exits 0
// instead of failing.
//
// Run with:  cargo run -p prism --example speak

use prism::{BackendFeatures, Context};

fn main() -> prism::Result<()> {
    let ctx = Context::new()?;

    println!(
        "Prism reports {} registered backend(s):",
        ctx.backend_count()
    );
    for i in 0..ctx.backend_count() {
        if let Some(id) = ctx.id_at(i) {
            let name = ctx.name_of(id).unwrap_or_else(|| "<unnamed>".into());
            println!("  [{i}] {name} (priority {})", ctx.priority_of(id));
        }
    }

    // `acquire_best` fails when no backend is actually available at runtime.
    let mut backend = match ctx.acquire_best() {
        Ok(b) => b,
        Err(e) => {
            println!("\nNo speech backend available on this machine ({e}).");
            println!("That's expected on headless systems — nothing to demo.");
            return Ok(());
        }
    };

    let features = backend.features();
    println!("\nUsing backend: {}", backend.name()?);
    println!("Advertised capabilities: {features:?}");

    // Only touch a capability the backend says it supports.
    if features.contains(BackendFeatures::SUPPORTS_SET_VOLUME) {
        backend.set_volume(0.8)?;
    }
    if features.contains(BackendFeatures::SUPPORTS_SET_RATE) {
        backend.set_rate(0.5)?;
    }

    if features.contains(BackendFeatures::SUPPORTS_COUNT_VOICES) {
        let count = backend.voice_count()?;
        println!("Backend exposes {count} voice(s).");
        if features.contains(BackendFeatures::SUPPORTS_GET_VOICE_NAME) {
            for v in 0..count.min(5) {
                let name = backend.voice_name(v).unwrap_or_default();
                println!("  voice {v}: {name}");
            }
        }
    }

    if features.contains(BackendFeatures::SUPPORTS_SPEAK) {
        println!("\nSpeaking...");
        backend.speak("Hello from Rust, spoken through Prism.", true)?;
    } else {
        println!("\nThis backend does not support speaking; nothing to say.");
    }

    Ok(())
}
