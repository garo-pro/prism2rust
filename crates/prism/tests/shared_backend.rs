// SPDX-License-Identifier: MPL-2.0
//! Soundness of shared (acquired) backends and of custom-backend registration.
//!
//! Upstream forbids concurrent calls on one backend instance, and every
//! context bound to the same registry gets the same cached instance from
//! `acquire`. These tests drive real cross-thread aliasing through the C
//! library and assert the wrapper serializes it, plus two registration
//! contracts: operation slots must match the declared features, and the
//! factory is freed exactly once even when registration fails.

use prism::{BackendFeatures, Context, CustomBackend, Error, RegistryBuilder, Result};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

/// Records whether two calls were ever inside the instance at the same time.
#[derive(Default)]
struct Probe {
    in_call: AtomicBool,
    overlapped: AtomicBool,
    calls: AtomicUsize,
}

struct ProbeBackend {
    probe: Arc<Probe>,
    rate: f32,
}

impl ProbeBackend {
    fn enter(&self) {
        if self.probe.in_call.swap(true, Ordering::SeqCst) {
            self.probe.overlapped.store(true, Ordering::SeqCst);
        }
        // Widen the window so unsynchronized callers would collide.
        thread::sleep(Duration::from_millis(5));
        self.probe.calls.fetch_add(1, Ordering::SeqCst);
        self.probe.in_call.store(false, Ordering::SeqCst);
    }
}

impl CustomBackend for ProbeBackend {
    fn speak(&mut self, _text: &str, _interrupt: bool) -> Result<()> {
        self.enter();
        Ok(())
    }
    fn set_rate(&mut self, rate: f32) -> Result<()> {
        self.rate = rate;
        Ok(())
    }
    fn get_rate(&self) -> Result<f32> {
        Ok(self.rate)
    }
}

fn probe_features() -> BackendFeatures {
    BackendFeatures::SUPPORTS_SPEAK
        | BackendFeatures::SUPPORTS_SET_RATE
        | BackendFeatures::SUPPORTS_GET_RATE
}

#[test]
fn acquired_handles_on_different_threads_never_overlap() {
    let probe = Arc::new(Probe::default());
    let factory_probe = Arc::clone(&probe);
    let mut builder = RegistryBuilder::new().unwrap();
    let id = builder
        .add_backend("probe_shared", 1, probe_features(), move || ProbeBackend {
            probe: Arc::clone(&factory_probe),
            rate: 0.5,
        })
        .unwrap();
    let registry = builder.freeze().unwrap();

    // Two contexts on one registry share its backend cache.
    let ctx_a = Context::builder()
        .registry(registry.clone())
        .build()
        .unwrap();
    let ctx_b = Context::builder().registry(registry).build().unwrap();
    let mut a = ctx_a.acquire(id).unwrap();
    let mut b = ctx_b.acquire(id).unwrap();

    // Prove the handles really alias one instance, so the test means something.
    a.set_rate(0.25).unwrap();
    assert_eq!(b.rate().unwrap(), 0.25, "handles do not share an instance");

    const ROUNDS: usize = 20;
    let start = Arc::new(Barrier::new(2));
    let start_b = Arc::clone(&start);
    let worker = thread::spawn(move || {
        start_b.wait();
        for _ in 0..ROUNDS {
            b.speak("from b", false).unwrap();
        }
    });
    start.wait();
    for _ in 0..ROUNDS {
        a.speak("from a", false).unwrap();
    }
    worker.join().unwrap();

    assert_eq!(probe.calls.load(Ordering::SeqCst), 2 * ROUNDS);
    assert!(
        !probe.overlapped.load(Ordering::SeqCst),
        "two threads were inside one backend instance at once"
    );
}

#[test]
fn backend_declaring_a_feature_subset_registers() {
    // Only the declared operations get vtable slots; upstream rejects a
    // registration whose slots and features disagree.
    let probe = Arc::new(Probe::default());
    let mut builder = RegistryBuilder::new().unwrap();
    let id = builder
        .add_backend("probe_subset", 1, probe_features(), move || ProbeBackend {
            probe: Arc::clone(&probe),
            rate: 0.5,
        })
        .expect("a feature subset must be accepted");
    let ctx = Context::builder()
        .registry(builder.freeze().unwrap())
        .build()
        .unwrap();
    let mut backend = ctx.create(id).unwrap();
    assert_eq!(backend.features() & probe_features(), probe_features());
    assert!(!backend
        .features()
        .contains(BackendFeatures::SUPPORTS_BRAILLE));
    backend.speak("hi", false).unwrap();
    // An undeclared operation never reaches the backend.
    assert_eq!(backend.braille("dots"), Err(Error::NotImplemented));
}

/// Counts drops of the value captured by a registration's factory.
struct DropCounter(Arc<AtomicUsize>);

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn failed_registration_frees_the_factory_exactly_once() {
    let drops = Arc::new(AtomicUsize::new(0));
    let counter = DropCounter(Arc::clone(&drops));
    let mut builder = RegistryBuilder::new().unwrap();
    // A backend cannot be both a screen reader and a speech engine, so
    // upstream rejects this and frees the factory itself.
    let invalid =
        probe_features() | BackendFeatures::IS_SCREEN_READER | BackendFeatures::IS_SPEECH_ENGINE;
    let result = builder.add_backend("probe_rejected", 1, invalid, move || {
        let _keep = &counter;
        ProbeBackend {
            probe: Arc::default(),
            rate: 0.5,
        }
    });
    assert!(result.is_err());
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "factory not freed exactly once"
    );
}
