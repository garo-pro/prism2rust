// SPDX-License-Identifier: MPL-2.0
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Pure-Rust, dependency-light shared types for the Prism bindings.
//!
//! This crate contains **only value types** mirrored from the public C header
//! `include/prism.h` of the pinned upstream Prism release (see `PRISM_PIN.toml`
//! at the repo root). It performs **no native linkage**, so its logic is fully
//! unit-testable on any platform without building the C/C++ library.
//!
//! Both [`prism-sys`](../prism_sys/index.html) (raw FFI) and the safe `prism`
//! crate re-use these types, guaranteeing a single definition of every error
//! code, backend id, and feature bit.
//!
//! Every constant here is asserted against the C header by the tests in this
//! crate; the `update-bridge` maintenance skill re-checks them when the pinned
//! upstream version changes.

use core::fmt;

/// Version of the [`PrismConfig`](../prism_sys/struct.PrismConfig.html) ABI this
/// binding targets. Mirrors `PRISM_CONFIG_VERSION`.
pub const CONFIG_VERSION: u8 = 3;

/// Backend-plugin ABI version this binding targets. Mirrors
/// `PRISM_PLUGIN_ABI_VERSION`.
///
/// Surfaced here rather than through the generated FFI because the C macro is
/// written as `UINT64_C(1)`, which bindgen does not evaluate. A plugin library
/// whose `abi_version` differs makes Prism report
/// [`Error::IncompatibleAbi`].
pub const PLUGIN_ABI_VERSION: u64 = 1;

/// Raw `PrismError` integer codes, exactly as defined by the C enum.
///
/// These are the wire values crossing the FFI boundary. Prefer the [`Error`]
/// enum for Rust-side handling; these constants exist so the mapping is
/// explicit and testable.
pub mod error_code {
    /// `PRISM_OK`
    pub const OK: i32 = 0;
    /// `PRISM_ERROR_NOT_INITIALIZED`
    pub const NOT_INITIALIZED: i32 = 1;
    /// `PRISM_ERROR_INVALID_PARAM`
    pub const INVALID_PARAM: i32 = 2;
    /// `PRISM_ERROR_NOT_IMPLEMENTED`
    pub const NOT_IMPLEMENTED: i32 = 3;
    /// `PRISM_ERROR_NO_VOICES`
    pub const NO_VOICES: i32 = 4;
    /// `PRISM_ERROR_VOICE_NOT_FOUND`
    pub const VOICE_NOT_FOUND: i32 = 5;
    /// `PRISM_ERROR_SPEAK_FAILURE`
    pub const SPEAK_FAILURE: i32 = 6;
    /// `PRISM_ERROR_MEMORY_FAILURE`
    pub const MEMORY_FAILURE: i32 = 7;
    /// `PRISM_ERROR_RANGE_OUT_OF_BOUNDS`
    pub const RANGE_OUT_OF_BOUNDS: i32 = 8;
    /// `PRISM_ERROR_INTERNAL`
    pub const INTERNAL: i32 = 9;
    /// `PRISM_ERROR_NOT_SPEAKING`
    pub const NOT_SPEAKING: i32 = 10;
    /// `PRISM_ERROR_NOT_PAUSED`
    pub const NOT_PAUSED: i32 = 11;
    /// `PRISM_ERROR_ALREADY_PAUSED`
    pub const ALREADY_PAUSED: i32 = 12;
    /// `PRISM_ERROR_INVALID_UTF8`
    pub const INVALID_UTF8: i32 = 13;
    /// `PRISM_ERROR_INVALID_OPERATION`
    pub const INVALID_OPERATION: i32 = 14;
    /// `PRISM_ERROR_ALREADY_INITIALIZED`
    pub const ALREADY_INITIALIZED: i32 = 15;
    /// `PRISM_ERROR_BACKEND_NOT_AVAILABLE`
    pub const BACKEND_NOT_AVAILABLE: i32 = 16;
    /// `PRISM_ERROR_UNKNOWN`
    pub const UNKNOWN: i32 = 17;
    /// `PRISM_ERROR_INVALID_AUDIO_FORMAT`
    pub const INVALID_AUDIO_FORMAT: i32 = 18;
    /// `PRISM_ERROR_INTERNAL_BACKEND_LIMIT_EXCEEDED`
    pub const INTERNAL_BACKEND_LIMIT_EXCEEDED: i32 = 19;
    /// `PRISM_ERROR_BACKEND_ENTERED_UNDEFINED_STATE`
    pub const BACKEND_ENTERED_UNDEFINED_STATE: i32 = 20;
    /// `PRISM_ERROR_LIBRARY_LOAD_FAILED`
    pub const LIBRARY_LOAD_FAILED: i32 = 21;
    /// `PRISM_ERROR_LIBRARY_INVALID`
    pub const LIBRARY_INVALID: i32 = 22;
    /// `PRISM_ERROR_INCOMPATIBLE_ABI`
    pub const INCOMPATIBLE_ABI: i32 = 23;
    /// `PRISM_ERROR_COUNT` — one past the last valid error code.
    pub const COUNT: i32 = 24;
}

/// A Prism error, mapped from a non-zero `PrismError` code.
///
/// `PRISM_OK` (`0`) is deliberately *not* representable: success is modeled as
/// `Ok(())` in [`Error::check`]. Any code outside the known range is preserved
/// via [`Error::Unrecognized`] so forward-compatibility never loses
/// information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The library or backend was used before initialization.
    NotInitialized,
    /// A parameter was invalid (null, empty, out of range, ...).
    InvalidParam,
    /// The requested operation is not implemented by this backend.
    NotImplemented,
    /// No voices are available.
    NoVoices,
    /// The requested voice does not exist.
    VoiceNotFound,
    /// Speaking failed.
    SpeakFailure,
    /// A memory allocation failed.
    MemoryFailure,
    /// An index or value was out of bounds.
    RangeOutOfBounds,
    /// An internal (non-recoverable) error occurred.
    Internal,
    /// The backend is not currently speaking.
    NotSpeaking,
    /// The backend is not currently paused.
    NotPaused,
    /// The backend is already paused.
    AlreadyPaused,
    /// A string was not valid UTF-8.
    InvalidUtf8,
    /// The operation is invalid in the current state.
    InvalidOperation,
    /// The library or backend was already initialized.
    AlreadyInitialized,
    /// The backend exists but is not available at runtime.
    BackendNotAvailable,
    /// An unknown error occurred.
    Unknown,
    /// The audio format reported by the backend is invalid.
    InvalidAudioFormat,
    /// An internal backend limit was exceeded.
    InternalBackendLimitExceeded,
    /// The backend entered an undefined state.
    BackendEnteredUndefinedState,
    /// A backend plugin library could not be loaded.
    LibraryLoadFailed,
    /// A backend plugin library loaded but is not a valid Prism plugin.
    LibraryInvalid,
    /// A backend plugin library declares an incompatible plugin ABI version.
    ///
    /// See [`PLUGIN_ABI_VERSION`] for the version this binding targets.
    IncompatibleAbi,
    /// A code outside the range known to this binding version.
    ///
    /// This is emitted when a newer upstream Prism returns an error code this
    /// binding has not been updated for. Run the `update-bridge` skill.
    Unrecognized(i32),
}

impl Error {
    /// Convert a raw `PrismError` code into a [`Result`].
    ///
    /// `0` (`PRISM_OK`) becomes `Ok(())`; every other value becomes the
    /// corresponding `Err(Error)`.
    #[inline]
    pub const fn check(code: i32) -> Result<(), Error> {
        match Self::from_code(code) {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Map a raw `PrismError` code to an [`Error`], or `None` for `PRISM_OK`.
    pub const fn from_code(code: i32) -> Option<Error> {
        use error_code as c;
        Some(match code {
            c::OK => return None,
            c::NOT_INITIALIZED => Error::NotInitialized,
            c::INVALID_PARAM => Error::InvalidParam,
            c::NOT_IMPLEMENTED => Error::NotImplemented,
            c::NO_VOICES => Error::NoVoices,
            c::VOICE_NOT_FOUND => Error::VoiceNotFound,
            c::SPEAK_FAILURE => Error::SpeakFailure,
            c::MEMORY_FAILURE => Error::MemoryFailure,
            c::RANGE_OUT_OF_BOUNDS => Error::RangeOutOfBounds,
            c::INTERNAL => Error::Internal,
            c::NOT_SPEAKING => Error::NotSpeaking,
            c::NOT_PAUSED => Error::NotPaused,
            c::ALREADY_PAUSED => Error::AlreadyPaused,
            c::INVALID_UTF8 => Error::InvalidUtf8,
            c::INVALID_OPERATION => Error::InvalidOperation,
            c::ALREADY_INITIALIZED => Error::AlreadyInitialized,
            c::BACKEND_NOT_AVAILABLE => Error::BackendNotAvailable,
            c::UNKNOWN => Error::Unknown,
            c::INVALID_AUDIO_FORMAT => Error::InvalidAudioFormat,
            c::INTERNAL_BACKEND_LIMIT_EXCEEDED => Error::InternalBackendLimitExceeded,
            c::BACKEND_ENTERED_UNDEFINED_STATE => Error::BackendEnteredUndefinedState,
            c::LIBRARY_LOAD_FAILED => Error::LibraryLoadFailed,
            c::LIBRARY_INVALID => Error::LibraryInvalid,
            c::INCOMPATIBLE_ABI => Error::IncompatibleAbi,
            other => Error::Unrecognized(other),
        })
    }

    /// The raw `PrismError` code this error maps to.
    pub const fn code(&self) -> i32 {
        use error_code as c;
        match self {
            Error::NotInitialized => c::NOT_INITIALIZED,
            Error::InvalidParam => c::INVALID_PARAM,
            Error::NotImplemented => c::NOT_IMPLEMENTED,
            Error::NoVoices => c::NO_VOICES,
            Error::VoiceNotFound => c::VOICE_NOT_FOUND,
            Error::SpeakFailure => c::SPEAK_FAILURE,
            Error::MemoryFailure => c::MEMORY_FAILURE,
            Error::RangeOutOfBounds => c::RANGE_OUT_OF_BOUNDS,
            Error::Internal => c::INTERNAL,
            Error::NotSpeaking => c::NOT_SPEAKING,
            Error::NotPaused => c::NOT_PAUSED,
            Error::AlreadyPaused => c::ALREADY_PAUSED,
            Error::InvalidUtf8 => c::INVALID_UTF8,
            Error::InvalidOperation => c::INVALID_OPERATION,
            Error::AlreadyInitialized => c::ALREADY_INITIALIZED,
            Error::BackendNotAvailable => c::BACKEND_NOT_AVAILABLE,
            Error::Unknown => c::UNKNOWN,
            Error::InvalidAudioFormat => c::INVALID_AUDIO_FORMAT,
            Error::InternalBackendLimitExceeded => c::INTERNAL_BACKEND_LIMIT_EXCEEDED,
            Error::BackendEnteredUndefinedState => c::BACKEND_ENTERED_UNDEFINED_STATE,
            Error::LibraryLoadFailed => c::LIBRARY_LOAD_FAILED,
            Error::LibraryInvalid => c::LIBRARY_INVALID,
            Error::IncompatibleAbi => c::INCOMPATIBLE_ABI,
            Error::Unrecognized(v) => *v,
        }
    }

    /// A stable, English fallback message.
    ///
    /// The safe crate prefers `prism_error_string()` from the C library; this
    /// is used when the raw C string is unavailable and for `Display`.
    pub const fn message(&self) -> &'static str {
        match self {
            Error::NotInitialized => "Prism is not initialized",
            Error::InvalidParam => "invalid parameter",
            Error::NotImplemented => "operation not implemented by this backend",
            Error::NoVoices => "no voices available",
            Error::VoiceNotFound => "voice not found",
            Error::SpeakFailure => "speak failed",
            Error::MemoryFailure => "memory allocation failed",
            Error::RangeOutOfBounds => "index or value out of bounds",
            Error::Internal => "internal error",
            Error::NotSpeaking => "backend is not speaking",
            Error::NotPaused => "backend is not paused",
            Error::AlreadyPaused => "backend is already paused",
            Error::InvalidUtf8 => "string is not valid UTF-8",
            Error::InvalidOperation => "invalid operation in the current state",
            Error::AlreadyInitialized => "already initialized",
            Error::BackendNotAvailable => "backend is not available at runtime",
            Error::Unknown => "unknown error",
            Error::InvalidAudioFormat => "invalid audio format reported by backend",
            Error::InternalBackendLimitExceeded => "internal backend limit exceeded",
            Error::BackendEnteredUndefinedState => "backend entered an undefined state",
            Error::LibraryLoadFailed => "backend plugin library failed to load",
            Error::LibraryInvalid => "backend plugin library is not a valid Prism plugin",
            Error::IncompatibleAbi => "backend plugin library has an incompatible ABI version",
            Error::Unrecognized(_) => "unrecognized Prism error code",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unrecognized(code) => write!(f, "{} ({code})", self.message()),
            _ => write!(f, "{} (code {})", self.message(), self.code()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Log verbosity, mirroring `PrismLogLevel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum LogLevel {
    /// `PRISM_LOG_LEVEL_TRACE`
    Trace = 0,
    /// `PRISM_LOG_LEVEL_DEBUG`
    Debug = 1,
    /// `PRISM_LOG_LEVEL_INFO`
    Info = 2,
    /// `PRISM_LOG_LEVEL_WARN`
    Warn = 3,
    /// `PRISM_LOG_LEVEL_ERROR`
    Error = 4,
    /// `PRISM_LOG_LEVEL_NONE`
    None = 5,
}

impl LogLevel {
    /// Map a raw `PrismLogLevel` integer to a [`LogLevel`], if in range.
    pub const fn from_i32(v: i32) -> Option<LogLevel> {
        Some(match v {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            4 => LogLevel::Error,
            5 => LogLevel::None,
            _ => return None,
        })
    }

    /// The raw `PrismLogLevel` integer value.
    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// A backend identifier (`PrismBackendId`), a stable 64-bit tag.
///
/// Known backends are exposed as associated constants; unknown/custom ids
/// (e.g. those minted by a [registry builder](../prism/index.html)) round-trip
/// losslessly through the [`u64`] value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BackendId(pub u64);

impl BackendId {
    /// `PRISM_BACKEND_INVALID` — the sentinel "no backend" id.
    pub const INVALID: BackendId = BackendId(0);
    /// `PRISM_BACKEND_SAPI`
    pub const SAPI: BackendId = BackendId(0x1D6D_F724_22CE_EE66);
    /// `PRISM_BACKEND_AV_SPEECH`
    pub const AV_SPEECH: BackendId = BackendId(0x28E3_4295_7780_5C24);
    /// `PRISM_BACKEND_VOICE_OVER`
    pub const VOICE_OVER: BackendId = BackendId(0xCB48_9796_1A75_4BCB);
    /// `PRISM_BACKEND_SPEECH_DISPATCHER`
    pub const SPEECH_DISPATCHER: BackendId = BackendId(0xE3D6_F895_D949_EBFE);
    /// `PRISM_BACKEND_NVDA`
    pub const NVDA: BackendId = BackendId(0x89CC_19C5_C4AC_1A56);
    /// `PRISM_BACKEND_JAWS`
    pub const JAWS: BackendId = BackendId(0xAC3D_60E9_BD84_B53E);
    /// `PRISM_BACKEND_ONE_CORE`
    pub const ONE_CORE: BackendId = BackendId(0x6797_D32F_0D99_4CB4);
    /// `PRISM_BACKEND_ORCA`
    pub const ORCA: BackendId = BackendId(0x10AA_1FC0_5A17_F96C);
    /// `PRISM_BACKEND_ANDROID_SCREEN_READER`
    pub const ANDROID_SCREEN_READER: BackendId = BackendId(0xD199_C175_AEEC_494B);
    /// `PRISM_BACKEND_ANDROID_TTS`
    pub const ANDROID_TTS: BackendId = BackendId(0xBC17_5831_BFE4_E5CC);
    /// `PRISM_BACKEND_WEB_SPEECH`
    pub const WEB_SPEECH: BackendId = BackendId(0x3572_538D_44D4_4A8F);
    /// `PRISM_BACKEND_UIA`
    pub const UIA: BackendId = BackendId(0x6238_F019_DB67_8F8E);
    /// `PRISM_BACKEND_ZDSR`
    pub const ZDSR: BackendId = BackendId(0x3D93_C56C_9E7F_2A2E);
    /// `PRISM_BACKEND_ZOOM_TEXT`
    pub const ZOOM_TEXT: BackendId = BackendId(0xAE43_9D62_DC7B_1479);
    /// `PRISM_BACKEND_BOY_PC_READER`
    pub const BOY_PC_READER: BackendId = BackendId(0x285A_BA1C_16F3_300F);
    /// `PRISM_BACKEND_PC_TALKER`
    pub const PC_TALKER: BackendId = BackendId(0x344B_9519_62E3_B835);
    /// `PRISM_BACKEND_SENSE_READER`
    pub const SENSE_READER: BackendId = BackendId(0xED47_6089_0B55_C2F2);
    /// `PRISM_BACKEND_SYSTEM_ACCESS`
    pub const SYSTEM_ACCESS: BackendId = BackendId(0x8380_F2A3_7B2C_3EB6);
    /// `PRISM_BACKEND_WINDOW_EYES`
    pub const WINDOW_EYES: BackendId = BackendId(0x9120_D899_0878_5C13);
    /// `PRISM_BACKEND_SPIEL`
    pub const SPIEL: BackendId = BackendId(0x478B_44F1_4AD3_D89C);

    /// The raw 64-bit id.
    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Whether this is a real id (i.e. not [`BackendId::INVALID`]).
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// The well-known human-readable name for this id, if it is a built-in.
    pub const fn well_known_name(self) -> Option<&'static str> {
        Some(match self.0 {
            x if x == Self::SAPI.0 => "SAPI",
            x if x == Self::AV_SPEECH.0 => "AVSpeech",
            x if x == Self::VOICE_OVER.0 => "VoiceOver",
            x if x == Self::SPEECH_DISPATCHER.0 => "Speech Dispatcher",
            x if x == Self::NVDA.0 => "NVDA",
            x if x == Self::JAWS.0 => "JAWS",
            x if x == Self::ONE_CORE.0 => "OneCore",
            x if x == Self::ORCA.0 => "Orca",
            x if x == Self::ANDROID_SCREEN_READER.0 => "Android Screen Reader",
            x if x == Self::ANDROID_TTS.0 => "Android TTS",
            x if x == Self::WEB_SPEECH.0 => "Web Speech",
            x if x == Self::UIA.0 => "UIA",
            x if x == Self::ZDSR.0 => "ZDSR",
            x if x == Self::ZOOM_TEXT.0 => "ZoomText",
            x if x == Self::BOY_PC_READER.0 => "BoyPCReader",
            x if x == Self::PC_TALKER.0 => "PC-Talker",
            x if x == Self::SENSE_READER.0 => "SenseReader",
            x if x == Self::SYSTEM_ACCESS.0 => "System Access",
            x if x == Self::WINDOW_EYES.0 => "Window-Eyes",
            x if x == Self::SPIEL.0 => "Spiel",
            _ => return None,
        })
    }

    /// Every built-in backend id, in header declaration order.
    pub const ALL: [BackendId; 20] = [
        Self::SAPI,
        Self::AV_SPEECH,
        Self::VOICE_OVER,
        Self::SPEECH_DISPATCHER,
        Self::NVDA,
        Self::JAWS,
        Self::ONE_CORE,
        Self::ORCA,
        Self::ANDROID_SCREEN_READER,
        Self::ANDROID_TTS,
        Self::WEB_SPEECH,
        Self::UIA,
        Self::ZDSR,
        Self::ZOOM_TEXT,
        Self::BOY_PC_READER,
        Self::PC_TALKER,
        Self::SENSE_READER,
        Self::SYSTEM_ACCESS,
        Self::WINDOW_EYES,
        Self::SPIEL,
    ];
}

impl From<u64> for BackendId {
    #[inline]
    fn from(v: u64) -> Self {
        BackendId(v)
    }
}

impl From<BackendId> for u64 {
    #[inline]
    fn from(v: BackendId) -> Self {
        v.0
    }
}

impl fmt::Display for BackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.well_known_name() {
            Some(name) => write!(f, "{name}"),
            None if !self.is_valid() => write!(f, "<invalid>"),
            None => write!(f, "<custom {:#018x}>", self.0),
        }
    }
}

bitflags::bitflags! {
    /// Capability flags reported by a backend (`PrismBackendFeature`).
    ///
    /// Bit positions match the C enum exactly (note bit 1 is intentionally
    /// unused upstream). A backend advertises which vtable entries are safe to
    /// call via these flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BackendFeatures: u64 {
        /// `PRISM_BACKEND_IS_SUPPORTED_AT_RUNTIME`
        const IS_SUPPORTED_AT_RUNTIME = 1 << 0;
        /// `PRISM_BACKEND_SUPPORTS_SPEAK`
        const SUPPORTS_SPEAK = 1 << 2;
        /// `PRISM_BACKEND_SUPPORTS_SPEAK_TO_MEMORY`
        const SUPPORTS_SPEAK_TO_MEMORY = 1 << 3;
        /// `PRISM_BACKEND_SUPPORTS_BRAILLE`
        const SUPPORTS_BRAILLE = 1 << 4;
        /// `PRISM_BACKEND_SUPPORTS_OUTPUT`
        const SUPPORTS_OUTPUT = 1 << 5;
        /// `PRISM_BACKEND_SUPPORTS_IS_SPEAKING`
        const SUPPORTS_IS_SPEAKING = 1 << 6;
        /// `PRISM_BACKEND_SUPPORTS_STOP`
        const SUPPORTS_STOP = 1 << 7;
        /// `PRISM_BACKEND_SUPPORTS_PAUSE`
        const SUPPORTS_PAUSE = 1 << 8;
        /// `PRISM_BACKEND_SUPPORTS_RESUME`
        const SUPPORTS_RESUME = 1 << 9;
        /// `PRISM_BACKEND_SUPPORTS_SET_VOLUME`
        const SUPPORTS_SET_VOLUME = 1 << 10;
        /// `PRISM_BACKEND_SUPPORTS_GET_VOLUME`
        const SUPPORTS_GET_VOLUME = 1 << 11;
        /// `PRISM_BACKEND_SUPPORTS_SET_RATE`
        const SUPPORTS_SET_RATE = 1 << 12;
        /// `PRISM_BACKEND_SUPPORTS_GET_RATE`
        const SUPPORTS_GET_RATE = 1 << 13;
        /// `PRISM_BACKEND_SUPPORTS_SET_PITCH`
        const SUPPORTS_SET_PITCH = 1 << 14;
        /// `PRISM_BACKEND_SUPPORTS_GET_PITCH`
        const SUPPORTS_GET_PITCH = 1 << 15;
        /// `PRISM_BACKEND_SUPPORTS_REFRESH_VOICES`
        const SUPPORTS_REFRESH_VOICES = 1 << 16;
        /// `PRISM_BACKEND_SUPPORTS_COUNT_VOICES`
        const SUPPORTS_COUNT_VOICES = 1 << 17;
        /// `PRISM_BACKEND_SUPPORTS_GET_VOICE_NAME`
        const SUPPORTS_GET_VOICE_NAME = 1 << 18;
        /// `PRISM_BACKEND_SUPPORTS_GET_VOICE_LANGUAGE`
        const SUPPORTS_GET_VOICE_LANGUAGE = 1 << 19;
        /// `PRISM_BACKEND_SUPPORTS_GET_VOICE`
        const SUPPORTS_GET_VOICE = 1 << 20;
        /// `PRISM_BACKEND_SUPPORTS_SET_VOICE`
        const SUPPORTS_SET_VOICE = 1 << 21;
        /// `PRISM_BACKEND_SUPPORTS_GET_CHANNELS`
        const SUPPORTS_GET_CHANNELS = 1 << 22;
        /// `PRISM_BACKEND_SUPPORTS_GET_SAMPLE_RATE`
        const SUPPORTS_GET_SAMPLE_RATE = 1 << 23;
        /// `PRISM_BACKEND_SUPPORTS_GET_BIT_DEPTH`
        const SUPPORTS_GET_BIT_DEPTH = 1 << 24;
        /// `PRISM_BACKEND_PERFORMS_SILENCE_TRIMMING_ON_SPEAK`
        const PERFORMS_SILENCE_TRIMMING_ON_SPEAK = 1 << 25;
        /// `PRISM_BACKEND_PERFORMS_SILENCE_TRIMMING_ON_SPEAK_TO_MEMORY`
        const PERFORMS_SILENCE_TRIMMING_ON_SPEAK_TO_MEMORY = 1 << 26;
        /// `PRISM_BACKEND_SUPPORTS_SPEAK_SSML`
        const SUPPORTS_SPEAK_SSML = 1 << 27;
        /// `PRISM_BACKEND_SUPPORTS_SPEAK_TO_MEMORY_SSML`
        const SUPPORTS_SPEAK_TO_MEMORY_SSML = 1 << 28;
        /// `PRISM_BACKEND_FEATURE_MAX_BIT` — highest reserved feature bit.
        const FEATURE_MAX_BIT = 1 << 63;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_code_is_success() {
        assert_eq!(Error::from_code(error_code::OK), None);
        assert_eq!(Error::check(error_code::OK), Ok(()));
    }

    #[test]
    fn every_known_code_round_trips() {
        // Every code in [1, COUNT) must map to a concrete variant whose
        // `.code()` returns the same integer.
        for code in 1..error_code::COUNT {
            let err = Error::from_code(code).expect("nonzero code -> Some");
            assert_eq!(err.code(), code, "round-trip mismatch for code {code}");
            assert_ne!(
                err,
                Error::Unrecognized(code),
                "code {code} should be a named variant, not Unrecognized"
            );
            assert_eq!(Error::check(code), Err(err));
        }
    }

    #[test]
    fn out_of_range_codes_are_preserved() {
        assert_eq!(Error::from_code(24), Some(Error::Unrecognized(24)));
        assert_eq!(Error::from_code(-5), Some(Error::Unrecognized(-5)));
        assert_eq!(Error::from_code(9999).unwrap().code(), 9999);
    }

    #[test]
    fn error_display_is_nonempty_and_has_code() {
        for code in 1..error_code::COUNT {
            let err = Error::from_code(code).unwrap();
            let s = format!("{err}");
            assert!(!s.is_empty());
            assert!(!err.message().is_empty());
        }
        assert!(format!("{}", Error::Unrecognized(42)).contains("42"));
    }

    #[test]
    fn log_level_round_trips() {
        for v in 0..=5 {
            let lvl = LogLevel::from_i32(v).expect("in range");
            assert_eq!(lvl.as_i32(), v);
        }
        assert_eq!(LogLevel::from_i32(6), None);
        assert_eq!(LogLevel::from_i32(-1), None);
        assert!(LogLevel::Trace < LogLevel::None);
    }

    #[test]
    fn backend_id_round_trips_and_names() {
        assert!(!BackendId::INVALID.is_valid());
        assert_eq!(BackendId::INVALID.well_known_name(), None);
        assert_eq!(BackendId::ALL.len(), 20);

        for id in BackendId::ALL {
            assert!(id.is_valid());
            assert!(id.well_known_name().is_some(), "{id:?} missing name");
            // u64 <-> BackendId round-trip.
            assert_eq!(BackendId::from(id.get()), id);
            assert_eq!(u64::from(id), id.0);
        }

        // Every built-in id is distinct.
        for (i, a) in BackendId::ALL.iter().enumerate() {
            for b in &BackendId::ALL[i + 1..] {
                assert_ne!(a, b, "duplicate backend id {a:?}");
            }
        }

        // A custom (unknown) id survives round-tripping and prints as custom.
        let custom = BackendId(0xDEAD_BEEF);
        assert!(custom.is_valid());
        assert_eq!(custom.well_known_name(), None);
        assert!(format!("{custom}").contains("custom"));
    }

    #[test]
    fn backend_id_spot_values_match_header() {
        // A couple of hand-verified anchors from include/prism.h.
        assert_eq!(BackendId::SAPI.0, 0x1D6DF72422CEEE66);
        assert_eq!(BackendId::NVDA.0, 0x89CC19C5C4AC1A56);
        assert_eq!(BackendId::SPIEL.0, 0x478B44F14AD3D89C);
        assert_eq!(BackendId::BOY_PC_READER.0, 0x285ABA1C16F3300F);
    }

    #[test]
    fn feature_bits_match_header_positions() {
        assert_eq!(BackendFeatures::IS_SUPPORTED_AT_RUNTIME.bits(), 1 << 0);
        // Bit 1 is intentionally skipped upstream.
        assert_eq!(BackendFeatures::SUPPORTS_SPEAK.bits(), 1 << 2);
        assert_eq!(BackendFeatures::SUPPORTS_SPEAK_TO_MEMORY.bits(), 1 << 3);
        assert_eq!(BackendFeatures::SUPPORTS_GET_BIT_DEPTH.bits(), 1 << 24);
        assert_eq!(BackendFeatures::SUPPORTS_SPEAK_SSML.bits(), 1 << 27);
        assert_eq!(
            BackendFeatures::SUPPORTS_SPEAK_TO_MEMORY_SSML.bits(),
            1 << 28
        );
        assert_eq!(BackendFeatures::FEATURE_MAX_BIT.bits(), 1 << 63);
    }

    #[test]
    fn feature_bitflags_compose_and_decompose() {
        let f = BackendFeatures::SUPPORTS_SPEAK | BackendFeatures::SUPPORTS_STOP;
        assert!(f.contains(BackendFeatures::SUPPORTS_SPEAK));
        assert!(!f.contains(BackendFeatures::SUPPORTS_BRAILLE));
        // Unknown/reserved bits round-trip via from_bits_retain.
        let raw = f.bits() | (1 << 1) | (1 << 40);
        let back = BackendFeatures::from_bits_retain(raw);
        assert_eq!(back.bits(), raw);
        assert!(back.contains(BackendFeatures::SUPPORTS_SPEAK));
    }

    #[test]
    fn config_version_matches_header() {
        assert_eq!(CONFIG_VERSION, 3);
    }

    #[test]
    fn plugin_abi_version_matches_header() {
        assert_eq!(PLUGIN_ABI_VERSION, 1);
    }

    #[test]
    fn plugin_error_codes_match_header() {
        // Hand-verified against include/prism.h at the pinned tag: the three
        // library-loading codes sit directly after BACKEND_ENTERED_UNDEFINED_STATE.
        assert_eq!(error_code::LIBRARY_LOAD_FAILED, 21);
        assert_eq!(error_code::LIBRARY_INVALID, 22);
        assert_eq!(error_code::INCOMPATIBLE_ABI, 23);
        assert_eq!(error_code::COUNT, 24);

        assert_eq!(
            Error::from_code(21).unwrap().code(),
            error_code::LIBRARY_LOAD_FAILED
        );
        assert_eq!(Error::from_code(22), Some(Error::LibraryInvalid));
        assert_eq!(Error::from_code(23), Some(Error::IncompatibleAbi));
    }
}
