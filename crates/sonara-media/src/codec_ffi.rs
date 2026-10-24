//! Codec FFI: the codec libraries, loaded at run time as replaceable dynamic libraries (LIC-01;
//! owner decisions of 2026-10-21 and 2026-10-22).
//!
//! This is the third module RT-10 allows `unsafe` in. It never runs on a real-time thread: the
//! decode cache and the exporter call it from worker threads.
//!
//! Three libraries are bound, each declared from its public headers, not from a binding crate
//! (`soxr`, `libsoxr-sys`, `mp3lame-sys` and `mp3lame-encoder` are LGPL themselves):
//!
//! - **libsoxr** (0.1.3's `soxr.h`), the resampler behind [`crate::ResampleStream`], for
//!   interleaved `f32` and `f64`;
//! - **libmp3lame** (LAME 3.100's `lame.h`), the MP3 encoder of the exporter;
//! - **libFLAC** (1.4.3's `FLAC/stream_encoder.h` and `FLAC/metadata.h`), the FLAC encoder of
//!   the exporter. It is BSD-licensed, but the prototype's files come from libFLAC 1.4.3, so the
//!   exporter drives that library instead of a Rust encoder to write the same bytes (GLD-01).
//!
//! No symbol is linked at build time, so a build without any of them works: resampling, MP3
//! and FLAC export report [`CodecError::LibraryNotFound`].
//!
//! A library is looked up under its versioned file name (`libsoxr.0.dylib`,
//! `libmp3lame.0.dylib` and `libFLAC.12.dylib` on macOS; `libsoxr.so.0`, `libmp3lame.so.0`
//! and `libFLAC.so.14` in the development container), in this order:
//!
//! 1. the app bundle: `Contents/Frameworks` next to `Contents/MacOS/<exe>`, filled by the
//!    packaging (macOS only);
//! 2. the directory in the `SONARA_NATIVE_LIB_DIR` environment variable, if it is set and not
//!    empty. Cargo sets it to `target/native/lib` (`.cargo/config.toml`), where
//!    `scripts/native/build-codecs.sh` builds the recording's libsoxr, LAME and libFLAC on
//!    macOS, so development, tests and CI load those. In the container the directory holds no
//!    `.so`, so the search goes on;
//! 3. the system's search path (the dynamic loader's own rules), which finds Debian's
//!    `libsoxr0`, `libmp3lame0` and `libflac14` in the container.
//!
//! A candidate that does not load is skipped and listed in [`CodecError::LibraryNotFound`].
//! Homebrew's prefixes are not searched: its libsoxr, LAME 4.0 and libFLAC are not the
//! recording's builds (GLD-01).

use std::ffi::{CStr, CString, OsString, c_char, c_float, c_int, c_uchar, c_uint, c_ulong, c_void};
use std::io::{self, Seek, Write};
use std::path::Path;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use libloading::Library;

/// A codec library is missing or failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodecError {
    /// The library could not be loaded from any candidate location.
    #[error("{library} is not available; tried {}", tried.join("; "))]
    LibraryNotFound {
        /// The library, for example `"libsoxr"`.
        library: &'static str,
        /// Each candidate with the loader's reason.
        tried: Vec<String>,
    },
    /// The library lacks a required function.
    #[error("{library} at {path} has no symbol {symbol}: {reason}")]
    MissingSymbol {
        /// The library.
        library: &'static str,
        /// The file that was loaded.
        path: String,
        /// The missing function.
        symbol: &'static str,
        /// The loader's reason.
        reason: String,
    },
    /// A library call reported an error.
    #[error("{library}: {operation} failed: {message}")]
    Call {
        /// The library.
        library: &'static str,
        /// The function that failed.
        operation: &'static str,
        /// The library's message.
        message: String,
    },
}

// ---------------------------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------------------------

/// Where one library is looked for.
struct LibrarySpec {
    /// The name in error messages, for example `"libsoxr"`.
    name: &'static str,
    /// The versioned file name on this platform.
    file: &'static str,
}

#[cfg(target_os = "macos")]
const SOXR: LibrarySpec = LibrarySpec {
    name: "libsoxr",
    file: "libsoxr.0.dylib",
};
#[cfg(windows)]
const SOXR: LibrarySpec = LibrarySpec {
    name: "libsoxr",
    file: "soxr.dll",
};
#[cfg(not(any(target_os = "macos", windows)))]
const SOXR: LibrarySpec = LibrarySpec {
    name: "libsoxr",
    file: "libsoxr.so.0",
};

#[cfg(target_os = "macos")]
const LAME: LibrarySpec = LibrarySpec {
    name: "libmp3lame",
    file: "libmp3lame.0.dylib",
};
#[cfg(windows)]
const LAME: LibrarySpec = LibrarySpec {
    name: "libmp3lame",
    file: "libmp3lame.dll",
};
#[cfg(not(any(target_os = "macos", windows)))]
const LAME: LibrarySpec = LibrarySpec {
    name: "libmp3lame",
    file: "libmp3lame.so.0",
};

#[cfg(target_os = "macos")]
const FLAC: LibrarySpec = LibrarySpec {
    name: "libFLAC",
    file: "libFLAC.12.dylib",
};
#[cfg(windows)]
const FLAC: LibrarySpec = LibrarySpec {
    name: "libFLAC",
    file: "FLAC.dll",
};
/// libFLAC 1.4.x has the soname 12; Debian trixie ships only libFLAC 1.5.0 (`libflac14`,
/// soname 14), whose stream-encoder and metadata-object API used here is the same. It is not
/// the recording's version, so the golden FLAC files are compared byte for byte on macOS only
/// (`tests/golden_export.rs`).
#[cfg(not(any(target_os = "macos", windows)))]
const FLAC: LibrarySpec = LibrarySpec {
    name: "libFLAC",
    file: "libFLAC.so.14",
};

/// The environment variable naming a directory of codec libraries, searched after the app
/// bundle and before the system (see the module documentation).
const NATIVE_LIB_DIR: &str = "SONARA_NATIVE_LIB_DIR";

/// The candidates for `spec` in the process's environment (see [`candidates`]).
fn candidates_from_env(spec: &LibrarySpec) -> Vec<OsString> {
    let native_dir = std::env::var_os(NATIVE_LIB_DIR);
    candidates(spec, native_dir.as_deref().map(Path::new))
}

/// The candidates for `spec`, in order: the app bundle, `native_dir` (the value of
/// [`NATIVE_LIB_DIR`]; ignored when empty), then the bare file name for the loader's search path.
fn candidates(spec: &LibrarySpec, native_dir: Option<&Path>) -> Vec<OsString> {
    let native = native_dir
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(spec.file).into_os_string());
    bundled(spec.file)
        .into_iter()
        .chain(native)
        .chain(std::iter::once(OsString::from(spec.file)))
        .collect()
}

/// `Sonara.app/Contents/Frameworks/<file>`, next to `Contents/MacOS/<exe>`.
#[cfg(target_os = "macos")]
fn bundled(file: &str) -> Option<OsString> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join("../Frameworks").join(file).into_os_string())
}

/// Only the macOS app bundle ships the codec libraries.
#[cfg(not(target_os = "macos"))]
fn bundled(_file: &str) -> Option<OsString> {
    None
}

/// Resolves one function of `library` as a plain function pointer.
///
/// # Safety
///
/// `T` must be the exact C signature of `name` in the loaded library, and the returned pointer
/// must only be called while `library` stays loaded.
unsafe fn symbol<T: Copy>(
    library: &Library,
    library_name: &'static str,
    path: &str,
    name: &'static str,
) -> Result<T, CodecError> {
    let mut c_name = Vec::with_capacity(name.len() + 1);
    c_name.extend_from_slice(name.as_bytes());
    c_name.push(0);
    // SAFETY: the caller guarantees that `T` is the symbol's signature; `c_name` is
    // NUL-terminated. Copying the function pointer out of the `Symbol` is sound because the
    // caller keeps `library` loaded for as long as the pointer is used.
    unsafe { library.get::<T>(c_name.as_slice()) }
        .map(|sym| *sym)
        .map_err(|err| CodecError::MissingSymbol {
            library: library_name,
            path: path.to_owned(),
            symbol: name,
            reason: err.to_string(),
        })
}

/// Loads the first candidate that loads, and resolves its functions with `resolve`. A file that
/// loads but lacks a function ends the search with [`CodecError::MissingSymbol`].
fn load_first<T>(
    library_name: &'static str,
    candidates: Vec<OsString>,
    resolve: fn(Library, &str) -> Result<T, CodecError>,
) -> Result<T, CodecError> {
    let mut tried = Vec::new();
    for candidate in candidates {
        // SAFETY: loading a shared library runs its initializers. libsoxr, libmp3lame and
        // libFLAC are plain C libraries whose initialization only sets up their own static
        // state; they register no handlers and start no threads at load time.
        match unsafe { Library::new(&candidate) } {
            Ok(library) => return resolve(library, &candidate.to_string_lossy()),
            Err(err) => tried.push(format!("{}: {err}", candidate.to_string_lossy())),
        }
    }
    Err(CodecError::LibraryNotFound {
        library: library_name,
        tried,
    })
}

/// Copies a C string returned by a library.
///
/// # Safety
///
/// `text` is null or points to a NUL-terminated string that stays valid during the call.
unsafe fn c_text(text: *const c_char) -> Option<String> {
    if text.is_null() {
        None
    } else {
        // SAFETY: non-null, and NUL-terminated per the caller's guarantee.
        Some(
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

// ---------------------------------------------------------------------------------------------
// libsoxr
// ---------------------------------------------------------------------------------------------

/// `soxr_datatype_t`, a C enum (`int`).
type SoxrDatatype = c_int;
/// `SOXR_FLOAT32_I`: interleaved 32-bit float.
const SOXR_FLOAT32_I: SoxrDatatype = 0;
/// `SOXR_FLOAT64_I`: interleaved 64-bit float.
const SOXR_FLOAT64_I: SoxrDatatype = 1;
/// `SOXR_VHQ` (`SOXR_28_BITQ`): the recipe python-soxr's `quality="VHQ"` selects.
const SOXR_VHQ: c_ulong = 6;

mod sealed {
    /// Only `f32` and `f64` are libsoxr sample types; the datatype tells libsoxr how wide a
    /// sample is, so no other type may implement this.
    pub trait Sealed: Copy + Default + Send + 'static {
        /// The interleaved `soxr_datatype_t`.
        const DATATYPE: super::SoxrDatatype;
    }

    impl Sealed for f32 {
        const DATATYPE: super::SoxrDatatype = super::SOXR_FLOAT32_I;
    }

    impl Sealed for f64 {
        const DATATYPE: super::SoxrDatatype = super::SOXR_FLOAT64_I;
    }
}

/// A sample type libsoxr resamples: `f32` (the decode cache) or `f64` (the exporter). Sealed.
pub trait SoxrSample: sealed::Sealed {}

impl SoxrSample for f32 {}
impl SoxrSample for f64 {}

/// `struct soxr_io_spec`.
#[repr(C)]
#[derive(Clone, Copy)]
struct SoxrIoSpec {
    itype: SoxrDatatype,
    otype: SoxrDatatype,
    scale: f64,
    e: *mut c_void,
    flags: c_ulong,
}

/// `struct soxr_quality_spec`.
#[repr(C)]
#[derive(Clone, Copy)]
struct SoxrQualitySpec {
    precision: f64,
    phase_response: f64,
    passband_end: f64,
    stopband_begin: f64,
    e: *mut c_void,
    flags: c_ulong,
}

/// `soxr_t`, a pointer to an opaque `struct soxr`.
type SoxrHandle = *mut c_void;
/// `soxr_error_t`: null for success, else a static, NUL-terminated message.
type SoxrErrorText = *const c_char;

type SoxrVersionFn = unsafe extern "C" fn() -> *const c_char;
type SoxrIoSpecFn = unsafe extern "C" fn(SoxrDatatype, SoxrDatatype) -> SoxrIoSpec;
type SoxrQualitySpecFn = unsafe extern "C" fn(c_ulong, c_ulong) -> SoxrQualitySpec;
type SoxrCreateFn = unsafe extern "C" fn(
    f64,
    f64,
    c_uint,
    *mut SoxrErrorText,
    *const SoxrIoSpec,
    *const SoxrQualitySpec,
    *const c_void,
) -> SoxrHandle;
type SoxrProcessFn = unsafe extern "C" fn(
    SoxrHandle,
    *const c_void,
    usize,
    *mut usize,
    *mut c_void,
    usize,
    *mut usize,
) -> SoxrErrorText;
type SoxrDelayFn = unsafe extern "C" fn(SoxrHandle) -> f64;
type SoxrEngineFn = unsafe extern "C" fn(SoxrHandle) -> *const c_char;
type SoxrDeleteFn = unsafe extern "C" fn(SoxrHandle);

/// The resolved libsoxr functions. The function pointers are valid while `_library` is loaded;
/// every user holds the table through an `Arc`, so they cannot outlive it.
struct SoxrApi {
    version: SoxrVersionFn,
    io_spec: SoxrIoSpecFn,
    quality_spec: SoxrQualitySpecFn,
    create: SoxrCreateFn,
    process: SoxrProcessFn,
    delay: SoxrDelayFn,
    engine: SoxrEngineFn,
    delete: SoxrDeleteFn,
    _library: Library,
}

impl SoxrApi {
    /// Resolves every function the wrapper uses.
    fn resolve(library: Library, path: &str) -> Result<Self, CodecError> {
        let n = SOXR.name;
        // SAFETY: every signature below is the one declared in libsoxr 0.1.3's `soxr.h` (the
        // API has been stable since 0.1.0), and the pointers live in the same `SoxrApi` as
        // `library`, which keeps the library loaded while they can be called.
        unsafe {
            Ok(Self {
                version: symbol(&library, n, path, "soxr_version")?,
                io_spec: symbol(&library, n, path, "soxr_io_spec")?,
                quality_spec: symbol(&library, n, path, "soxr_quality_spec")?,
                create: symbol(&library, n, path, "soxr_create")?,
                process: symbol(&library, n, path, "soxr_process")?,
                delay: symbol(&library, n, path, "soxr_delay")?,
                engine: symbol(&library, n, path, "soxr_engine")?,
                delete: symbol(&library, n, path, "soxr_delete")?,
                _library: library,
            })
        }
    }
}

/// A loaded libsoxr, shared by every resampler.
#[derive(Clone)]
pub(crate) struct SoxrLibrary(Arc<SoxrApi>);

impl SoxrLibrary {
    /// The process-wide libsoxr, loaded on first use from the candidate locations.
    ///
    /// # Errors
    ///
    /// [`CodecError::LibraryNotFound`] when no candidate loads (the result is remembered), or
    /// [`CodecError::MissingSymbol`] when the loaded file is not a compatible libsoxr.
    pub(crate) fn shared() -> Result<Self, CodecError> {
        static LIBRARY: OnceLock<Result<SoxrLibrary, CodecError>> = OnceLock::new();
        LIBRARY
            .get_or_init(|| Self::load_first(candidates_from_env(&SOXR)))
            .clone()
    }

    /// Loads the first candidate that is a loadable libsoxr.
    fn load_first(candidates: Vec<OsString>) -> Result<Self, CodecError> {
        load_first(SOXR.name, candidates, SoxrApi::resolve).map(|api| Self(Arc::new(api)))
    }

    /// `soxr_version()`, for example `"libsoxr-0.1.3"`.
    pub(crate) fn version(&self) -> String {
        // SAFETY: `soxr_version` takes no arguments and returns a pointer to a static,
        // NUL-terminated string.
        unsafe { c_text((self.0.version)()) }.unwrap_or_default()
    }
}

/// Serializes every libsoxr call that can touch its process-wide FFT tables.
///
/// libsoxr keeps the DFT tables in globals (`lsx_fft_br`, `lsx_fft_sc` in `fft4g_cache.h`) and
/// guards them with locks that are no-ops without OpenMP. The macOS build of
/// `scripts/native/build-codecs.sh`, like python-soxr's own build, has no OpenMP, so a stream
/// that needs a longer DFT `realloc`s the tables while other streams read them: concurrent
/// decode jobs or an import during an export could corrupt each other's output. The calls run
/// on worker threads only, never on a real-time thread, so one lock is allowed here (MEM-08) and
/// keeps every stream's arithmetic unchanged.
static SOXR_LOCK: Mutex<()> = Mutex::new(());

/// Holds [`SOXR_LOCK`]; a panic while it was held leaves no state to repair, so poisoning is
/// ignored.
fn soxr_lock() -> MutexGuard<'static, ()> {
    SOXR_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One libsoxr stream resampler for interleaved samples of type `S`, configured as python-soxr
/// 1.1 configures `ResampleStream(…, dtype=…, quality="VHQ")`: io spec
/// `soxr_io_spec(T, T)` with `T` the interleaved type of `S`, quality spec
/// `soxr_quality_spec(SOXR_VHQ, 0)` and the default runtime spec (`NULL`). Dropping it calls
/// `soxr_delete` (MEM-07).
pub(crate) struct Soxr<S: SoxrSample> {
    library: SoxrLibrary,
    handle: NonNull<c_void>,
    channels: usize,
    _sample: std::marker::PhantomData<S>,
}

impl<S: SoxrSample> Soxr<S> {
    /// `soxr_create(in_rate, out_rate, channels, …)`.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] when libsoxr rejects the configuration, for example a zero rate.
    pub(crate) fn new_vhq(
        library: SoxrLibrary,
        in_rate: f64,
        out_rate: f64,
        channels: usize,
    ) -> Result<Self, CodecError> {
        let call_error = |message: String| CodecError::Call {
            library: SOXR.name,
            operation: "soxr_create",
            message,
        };
        let num_channels = c_uint::try_from(channels)
            .ok()
            .filter(|&n| n > 0)
            .ok_or_else(|| call_error(format!("invalid channel count {channels}")))?;
        let api = &library.0;
        let mut error: SoxrErrorText = ptr::null();
        // SAFETY: both constructors take plain values and return their structs by value; the
        // `#[repr(C)]` declarations match `soxr.h`.
        let (io_spec, quality_spec) = unsafe {
            (
                (api.io_spec)(S::DATATYPE, S::DATATYPE),
                (api.quality_spec)(SOXR_VHQ, 0),
            )
        };
        let _serialized = soxr_lock();
        // SAFETY: the spec pointers point to live locals for the duration of the call, the
        // runtime spec may be null (libsoxr then uses `soxr_runtime_spec(1)`), and `error` is a
        // valid out-pointer.
        let handle = unsafe {
            (api.create)(
                in_rate,
                out_rate,
                num_channels,
                &raw mut error,
                &raw const io_spec,
                &raw const quality_spec,
                ptr::null(),
            )
        };
        // SAFETY: `error` is null or one of libsoxr's static error texts.
        if let Some(message) = unsafe { c_text(error) } {
            if let Some(handle) = NonNull::new(handle) {
                // SAFETY: a handle returned by `soxr_create` is deleted exactly once.
                unsafe { (api.delete)(handle.as_ptr()) };
            }
            return Err(call_error(message));
        }
        let handle = NonNull::new(handle)
            .ok_or_else(|| call_error("soxr_create returned no resampler".to_owned()))?;
        Ok(Self {
            library,
            handle,
            channels,
            _sample: std::marker::PhantomData,
        })
    }

    /// `soxr_process` with interleaved frames. `None` as input signals the end of the input
    /// (flush); `Some` slices are consumed completely (`idone` is `NULL`, as python-soxr passes
    /// it). Returns the frames written to `output`.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] with libsoxr's message.
    pub(crate) fn process(
        &mut self,
        input: Option<&[S]>,
        output: &mut [S],
    ) -> Result<usize, CodecError> {
        let (in_ptr, in_frames) = match input {
            Some(samples) => (
                samples.as_ptr().cast::<c_void>(),
                samples.len() / self.channels,
            ),
            None => (ptr::null(), 0),
        };
        let out_frames = output.len() / self.channels;
        let mut done = 0usize;
        let _serialized = soxr_lock();
        // SAFETY: `handle` is a live resampler created for `channels` interleaved channels of
        // `S`, whose datatype the sealed `SoxrSample` fixes (`f32` ↔ `SOXR_FLOAT32_I`, `f64` ↔
        // `SOXR_FLOAT64_I`). The input pointer is null (flush) or points to
        // `in_frames * channels` readable samples, and the output pointer to
        // `out_frames * channels` writable samples, so libsoxr reads and writes within the
        // slices. `idone` may be null; `done` is a valid out-pointer.
        let error = unsafe {
            (self.library.0.process)(
                self.handle.as_ptr(),
                in_ptr,
                in_frames,
                ptr::null_mut(),
                output.as_mut_ptr().cast::<c_void>(),
                out_frames,
                &raw mut done,
            )
        };
        // SAFETY: `error` is null or a static libsoxr error text.
        match unsafe { c_text(error) } {
            Some(message) => Err(CodecError::Call {
                library: SOXR.name,
                operation: "soxr_process",
                message,
            }),
            None => Ok(done.min(out_frames)),
        }
    }

    /// `soxr_delay`: the output frames still pending inside the resampler.
    pub(crate) fn delay(&self) -> f64 {
        // SAFETY: `handle` is a live resampler.
        unsafe { (self.library.0.delay)(self.handle.as_ptr()) }
    }

    /// `soxr_engine`, for example `"cr64"`: the arithmetic libsoxr selected.
    pub(crate) fn engine(&self) -> String {
        // SAFETY: `handle` is a live resampler; the engine name is a static string, or null
        // in no released libsoxr.
        let name = unsafe { c_text((self.library.0.engine)(self.handle.as_ptr())) };
        name.unwrap_or_default()
    }
}

impl<S: SoxrSample> Drop for Soxr<S> {
    fn drop(&mut self) {
        let _serialized = soxr_lock();
        // SAFETY: `handle` came from `soxr_create` and is deleted exactly once, here.
        unsafe { (self.library.0.delete)(self.handle.as_ptr()) };
    }
}

// SAFETY: a libsoxr resampler has no thread affinity; the handle is owned exclusively by this
// value, and every call goes through `&mut self` or reads only (`soxr_delay`, `soxr_engine`).
unsafe impl<S: SoxrSample> Send for Soxr<S> {}

// ---------------------------------------------------------------------------------------------
// libmp3lame
// ---------------------------------------------------------------------------------------------

/// `lame_t` (`lame_global_flags *`).
type LameHandle = *mut c_void;

/// `vbr_mode` of `lame.h`, a C enum (`int`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VbrMode {
    /// `vbr_off`: constant bitrate.
    Off,
    /// `vbr_abr`: average bitrate.
    Abr,
    /// `vbr_default` (`vbr_mtrh`): variable bitrate.
    Default,
}

impl VbrMode {
    const fn to_c(self) -> c_int {
        match self {
            Self::Off => 0,
            Self::Abr => 3,
            Self::Default => 4,
        }
    }

    /// `lame_get_VBR`'s value; `vbr_mt` (1), `vbr_rh` (2) and `vbr_mtrh` (4) are variable.
    fn from_c(value: c_int) -> Option<Self> {
        match value {
            0 => Some(Self::Off),
            3 => Some(Self::Abr),
            1 | 2 | 4 => Some(Self::Default),
            _ => None,
        }
    }
}

type LameInitFn = unsafe extern "C" fn() -> LameHandle;
type LameSetIntFn = unsafe extern "C" fn(LameHandle, c_int) -> c_int;
type LameSetVoidFn = unsafe extern "C" fn(LameHandle, c_int);
type LameGetIntFn = unsafe extern "C" fn(LameHandle) -> c_int;
type LameSetFloatFn = unsafe extern "C" fn(LameHandle, c_float) -> c_int;
type LameInitParamsFn = unsafe extern "C" fn(LameHandle) -> c_int;
type Id3InitFn = unsafe extern "C" fn(LameHandle);
type Id3SetFn = unsafe extern "C" fn(LameHandle, *const c_char);
type Id3SetCheckedFn = unsafe extern "C" fn(LameHandle, *const c_char) -> c_int;
type LameGetTagFn = unsafe extern "C" fn(LameHandle, *mut c_uchar, usize) -> usize;
type LameEncodeInterleavedFloatFn =
    unsafe extern "C" fn(LameHandle, *const c_float, c_int, *mut c_uchar, c_int) -> c_int;
type LameEncodeFloatFn = unsafe extern "C" fn(
    LameHandle,
    *const c_float,
    *const c_float,
    c_int,
    *mut c_uchar,
    c_int,
) -> c_int;
type LameFlushFn = unsafe extern "C" fn(LameHandle, *mut c_uchar, c_int) -> c_int;
type LameCloseFn = unsafe extern "C" fn(LameHandle) -> c_int;
type LameVersionFn = unsafe extern "C" fn() -> *const c_char;

/// The resolved libmp3lame functions. The function pointers are valid while `_library` is
/// loaded; every user holds the table through an `Arc`, so they cannot outlive it.
struct LameApi {
    init: LameInitFn,
    set_in_samplerate: LameSetIntFn,
    set_num_channels: LameSetIntFn,
    set_out_samplerate: LameSetIntFn,
    set_write_id3tag_automatic: LameSetVoidFn,
    set_vbr: LameSetIntFn,
    get_vbr: LameGetIntFn,
    set_vbr_quality: LameSetFloatFn,
    set_brate: LameSetIntFn,
    set_vbr_mean_bitrate_kbps: LameSetIntFn,
    init_params: LameInitParamsFn,
    get_framesize: LameGetIntFn,
    id3tag_init: Id3InitFn,
    id3tag_set_title: Id3SetFn,
    id3tag_set_artist: Id3SetFn,
    id3tag_set_album: Id3SetFn,
    id3tag_set_year: Id3SetFn,
    id3tag_set_comment: Id3SetFn,
    id3tag_set_track: Id3SetCheckedFn,
    id3tag_set_genre: Id3SetCheckedFn,
    get_id3v2_tag: LameGetTagFn,
    get_id3v1_tag: LameGetTagFn,
    get_lametag_frame: LameGetTagFn,
    encode_interleaved_float: LameEncodeInterleavedFloatFn,
    encode_float: LameEncodeFloatFn,
    flush: LameFlushFn,
    close: LameCloseFn,
    version: LameVersionFn,
    _library: Library,
}

impl LameApi {
    /// Resolves every function the wrapper uses.
    fn resolve(library: Library, path: &str) -> Result<Self, CodecError> {
        let n = LAME.name;
        // SAFETY: every signature below is the one declared in LAME 3.100's `lame.h` (enums
        // are `int`, `lame_t` is a pointer, `size_t` is `usize`), and the pointers live in the
        // same `LameApi` as `library`, which keeps the library loaded while they can be called.
        unsafe {
            Ok(Self {
                init: symbol(&library, n, path, "lame_init")?,
                set_in_samplerate: symbol(&library, n, path, "lame_set_in_samplerate")?,
                set_num_channels: symbol(&library, n, path, "lame_set_num_channels")?,
                set_out_samplerate: symbol(&library, n, path, "lame_set_out_samplerate")?,
                set_write_id3tag_automatic: symbol(
                    &library,
                    n,
                    path,
                    "lame_set_write_id3tag_automatic",
                )?,
                set_vbr: symbol(&library, n, path, "lame_set_VBR")?,
                get_vbr: symbol(&library, n, path, "lame_get_VBR")?,
                set_vbr_quality: symbol(&library, n, path, "lame_set_VBR_quality")?,
                set_brate: symbol(&library, n, path, "lame_set_brate")?,
                set_vbr_mean_bitrate_kbps: symbol(
                    &library,
                    n,
                    path,
                    "lame_set_VBR_mean_bitrate_kbps",
                )?,
                init_params: symbol(&library, n, path, "lame_init_params")?,
                get_framesize: symbol(&library, n, path, "lame_get_framesize")?,
                id3tag_init: symbol(&library, n, path, "id3tag_init")?,
                id3tag_set_title: symbol(&library, n, path, "id3tag_set_title")?,
                id3tag_set_artist: symbol(&library, n, path, "id3tag_set_artist")?,
                id3tag_set_album: symbol(&library, n, path, "id3tag_set_album")?,
                id3tag_set_year: symbol(&library, n, path, "id3tag_set_year")?,
                id3tag_set_comment: symbol(&library, n, path, "id3tag_set_comment")?,
                id3tag_set_track: symbol(&library, n, path, "id3tag_set_track")?,
                id3tag_set_genre: symbol(&library, n, path, "id3tag_set_genre")?,
                get_id3v2_tag: symbol(&library, n, path, "lame_get_id3v2_tag")?,
                get_id3v1_tag: symbol(&library, n, path, "lame_get_id3v1_tag")?,
                get_lametag_frame: symbol(&library, n, path, "lame_get_lametag_frame")?,
                encode_interleaved_float: symbol(
                    &library,
                    n,
                    path,
                    "lame_encode_buffer_interleaved_ieee_float",
                )?,
                encode_float: symbol(&library, n, path, "lame_encode_buffer_ieee_float")?,
                flush: symbol(&library, n, path, "lame_encode_flush")?,
                close: symbol(&library, n, path, "lame_close")?,
                version: symbol(&library, n, path, "get_lame_version")?,
                _library: library,
            })
        }
    }
}

/// A loaded libmp3lame, shared by every encoder.
#[derive(Clone)]
pub(crate) struct LameLibrary(Arc<LameApi>);

impl LameLibrary {
    /// The process-wide libmp3lame, loaded on first use from the candidate locations.
    ///
    /// # Errors
    ///
    /// [`CodecError::LibraryNotFound`] when no candidate loads (the result is remembered), or
    /// [`CodecError::MissingSymbol`] when the loaded file is not a compatible libmp3lame.
    pub(crate) fn shared() -> Result<Self, CodecError> {
        static LIBRARY: OnceLock<Result<LameLibrary, CodecError>> = OnceLock::new();
        LIBRARY
            .get_or_init(|| Self::load_first(candidates_from_env(&LAME)))
            .clone()
    }

    /// Loads the first candidate that is a loadable libmp3lame.
    fn load_first(candidates: Vec<OsString>) -> Result<Self, CodecError> {
        load_first(LAME.name, candidates, LameApi::resolve).map(|api| Self(Arc::new(api)))
    }

    /// `get_lame_version()`, for example `"3.100"`.
    pub(crate) fn version(&self) -> String {
        // SAFETY: `get_lame_version` takes no arguments and returns a static string.
        unsafe { c_text((self.0.version)()) }.unwrap_or_default()
    }
}

/// One LAME encoder (`lame_t`). The methods are LAME's setters and encoders with bounds-checked
/// buffers; dropping it calls `lame_close` (MEM-07).
pub(crate) struct Lame {
    library: LameLibrary,
    handle: NonNull<c_void>,
}

/// LAME's `LAME_OKAY`.
const LAME_OKAY: c_int = 0;

impl Lame {
    /// `lame_init()`.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] when LAME cannot allocate an encoder.
    pub(crate) fn new(library: LameLibrary) -> Result<Self, CodecError> {
        // SAFETY: `lame_init` takes no arguments and returns a new encoder or null.
        let handle = unsafe { (library.0.init)() };
        let handle = NonNull::new(handle).ok_or_else(|| Self::error("lame_init", "no encoder"))?;
        Ok(Self { library, handle })
    }

    fn error(operation: &'static str, message: impl Into<String>) -> CodecError {
        CodecError::Call {
            library: LAME.name,
            operation,
            message: message.into(),
        }
    }

    fn api(&self) -> &LameApi {
        &self.library.0
    }

    fn checked(operation: &'static str, code: c_int) -> Result<(), CodecError> {
        if code == LAME_OKAY {
            Ok(())
        } else {
            Err(Self::error(operation, format!("returned {code}")))
        }
    }

    /// `lame_set_in_samplerate`.
    pub(crate) fn set_in_samplerate(&mut self, rate: c_int) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a plain value.
        let code = unsafe { (self.api().set_in_samplerate)(self.handle.as_ptr(), rate) };
        Self::checked("lame_set_in_samplerate", code)
    }

    /// `lame_set_num_channels`.
    pub(crate) fn set_num_channels(&mut self, channels: c_int) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a plain value.
        let code = unsafe { (self.api().set_num_channels)(self.handle.as_ptr(), channels) };
        Self::checked("lame_set_num_channels", code)
    }

    /// `lame_set_out_samplerate`; LAME reports a rate it cannot write with a negative code.
    pub(crate) fn set_out_samplerate(&mut self, rate: c_int) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a plain value.
        let code = unsafe { (self.api().set_out_samplerate)(self.handle.as_ptr(), rate) };
        if code < 0 {
            Err(Self::error(
                "lame_set_out_samplerate",
                format!("rate {rate} rejected"),
            ))
        } else {
            Ok(())
        }
    }

    /// `lame_set_write_id3tag_automatic`.
    pub(crate) fn set_write_id3tag_automatic(&mut self, automatic: bool) {
        // SAFETY: a live encoder and a plain value.
        unsafe {
            (self.api().set_write_id3tag_automatic)(self.handle.as_ptr(), c_int::from(automatic));
        }
    }

    /// `lame_set_VBR`.
    pub(crate) fn set_vbr(&mut self, mode: VbrMode) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a valid `vbr_mode` value.
        let code = unsafe { (self.api().set_vbr)(self.handle.as_ptr(), mode.to_c()) };
        Self::checked("lame_set_VBR", code)
    }

    /// `lame_get_VBR`; `None` for a value outside `vbr_mode`.
    pub(crate) fn vbr(&self) -> Option<VbrMode> {
        // SAFETY: a live encoder.
        VbrMode::from_c(unsafe { (self.api().get_vbr)(self.handle.as_ptr()) })
    }

    /// `lame_set_VBR_quality` (a C `float`).
    pub(crate) fn set_vbr_quality(&mut self, quality: f32) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a plain value.
        let code = unsafe { (self.api().set_vbr_quality)(self.handle.as_ptr(), quality) };
        Self::checked("lame_set_VBR_quality", code)
    }

    /// `lame_set_brate` (kbps).
    pub(crate) fn set_brate(&mut self, kbps: c_int) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a plain value.
        let code = unsafe { (self.api().set_brate)(self.handle.as_ptr(), kbps) };
        Self::checked("lame_set_brate", code)
    }

    /// `lame_set_VBR_mean_bitrate_kbps`.
    pub(crate) fn set_vbr_mean_bitrate_kbps(&mut self, kbps: c_int) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a plain value.
        let code = unsafe { (self.api().set_vbr_mean_bitrate_kbps)(self.handle.as_ptr(), kbps) };
        Self::checked("lame_set_VBR_mean_bitrate_kbps", code)
    }

    /// `lame_init_params`.
    pub(crate) fn init_params(&mut self) -> Result<(), CodecError> {
        // SAFETY: a live encoder.
        let code = unsafe { (self.api().init_params)(self.handle.as_ptr()) };
        if code < 0 {
            Err(Self::error("lame_init_params", format!("returned {code}")))
        } else {
            Ok(())
        }
    }

    /// `lame_get_framesize`: samples per channel in one MP3 frame.
    pub(crate) fn framesize(&self) -> c_int {
        // SAFETY: a live encoder.
        unsafe { (self.api().get_framesize)(self.handle.as_ptr()) }
    }

    /// `id3tag_init`.
    pub(crate) fn id3tag_init(&mut self) {
        // SAFETY: a live encoder.
        unsafe { (self.api().id3tag_init)(self.handle.as_ptr()) };
    }

    /// One of the `id3tag_set_*` functions. LAME copies the text. A value with an interior NUL
    /// is passed up to that NUL, as a C caller would see it.
    pub(crate) fn id3tag_set(&mut self, field: Id3Field, value: &[u8]) {
        let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
        let text = CString::new(value.get(..end).unwrap_or_default()).unwrap_or_default();
        let api = self.api();
        let handle = self.handle.as_ptr();
        // SAFETY: a live encoder and a NUL-terminated string that outlives the call; LAME
        // copies the text (`local_strdup`). The track and genre setters return a status that
        // libsndfile ignores, as this wrapper does.
        unsafe {
            match field {
                Id3Field::Title => (api.id3tag_set_title)(handle, text.as_ptr()),
                Id3Field::Artist => (api.id3tag_set_artist)(handle, text.as_ptr()),
                Id3Field::Album => (api.id3tag_set_album)(handle, text.as_ptr()),
                Id3Field::Year => (api.id3tag_set_year)(handle, text.as_ptr()),
                Id3Field::Comment => (api.id3tag_set_comment)(handle, text.as_ptr()),
                Id3Field::Track => {
                    (api.id3tag_set_track)(handle, text.as_ptr());
                }
                Id3Field::Genre => {
                    (api.id3tag_set_genre)(handle, text.as_ptr());
                }
            }
        }
    }

    /// Calls a "size, then fill" tag getter: the bytes it returns, empty when there is none.
    fn tag(&self, getter: LameGetTagFn) -> Vec<u8> {
        // SAFETY: a null buffer of size 0 asks for the size only.
        let size = unsafe { getter(self.handle.as_ptr(), ptr::null_mut(), 0) };
        if size == 0 {
            return Vec::new();
        }
        let mut buffer = vec![0u8; size];
        // SAFETY: `buffer` has room for `size` bytes, the size LAME asked for.
        let written = unsafe { getter(self.handle.as_ptr(), buffer.as_mut_ptr(), buffer.len()) };
        buffer.truncate(written.min(size));
        buffer
    }

    /// `lame_get_id3v2_tag`: the `ID3v2` tag, empty when LAME writes none.
    pub(crate) fn id3v2_tag(&self) -> Vec<u8> {
        self.tag(self.api().get_id3v2_tag)
    }

    /// `lame_get_id3v1_tag`: the 128-byte `ID3v1` trailer, empty when LAME writes none.
    pub(crate) fn id3v1_tag(&self) -> Vec<u8> {
        self.tag(self.api().get_id3v1_tag)
    }

    /// `lame_get_lametag_frame`: the Xing/Info frame with the LAME tag, empty when disabled.
    pub(crate) fn lametag_frame(&self) -> Vec<u8> {
        self.tag(self.api().get_lametag_frame)
    }

    /// `lame_encode_buffer_interleaved_ieee_float` with `frames` stereo frames from
    /// `samples` (2 × `frames` values); appends the encoded bytes to `out`.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] for a short input or a negative LAME code.
    pub(crate) fn encode_interleaved(
        &mut self,
        samples: &[f32],
        frames: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), CodecError> {
        if samples.len() < frames.saturating_mul(2) {
            return Err(Self::error("lame_encode_buffer", "short input"));
        }
        let frames_c = c_int::try_from(frames)
            .map_err(|_| Self::error("lame_encode_buffer", "too many frames"))?;
        let mut buffer = encode_buffer(frames);
        let size = c_int::try_from(buffer.len())
            .map_err(|_| Self::error("lame_encode_buffer", "buffer too large"))?;
        // SAFETY: a live stereo encoder; `samples` holds at least `2 * frames` readable floats
        // (checked above), and `buffer` has `size` writable bytes, LAME's documented worst
        // case `1.25 * frames + 7200`.
        let code = unsafe {
            (self.api().encode_interleaved_float)(
                self.handle.as_ptr(),
                samples.as_ptr(),
                frames_c,
                buffer.as_mut_ptr(),
                size,
            )
        };
        append_encoded(&buffer, code, out)
    }

    /// `lame_encode_buffer_ieee_float` with one channel (`pcm_r` is null): appends the encoded
    /// bytes to `out`.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] for a negative LAME code.
    pub(crate) fn encode_mono(
        &mut self,
        samples: &[f32],
        out: &mut Vec<u8>,
    ) -> Result<(), CodecError> {
        let frames = c_int::try_from(samples.len())
            .map_err(|_| Self::error("lame_encode_buffer", "too many frames"))?;
        let mut buffer = encode_buffer(samples.len());
        let size = c_int::try_from(buffer.len())
            .map_err(|_| Self::error("lame_encode_buffer", "buffer too large"))?;
        // SAFETY: a live mono encoder, which reads only the left channel (`pcm_r` may be null
        // for one channel); `samples` holds `frames` readable floats and `buffer` has `size`
        // writable bytes.
        let code = unsafe {
            (self.api().encode_float)(
                self.handle.as_ptr(),
                samples.as_ptr(),
                ptr::null(),
                frames,
                buffer.as_mut_ptr(),
                size,
            )
        };
        append_encoded(&buffer, code, out)
    }

    /// `lame_encode_flush` into a 7200-byte buffer (the size `lame.h` documents): appends the
    /// last frames to `out`.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] for a negative LAME code.
    pub(crate) fn flush(&mut self, out: &mut Vec<u8>) -> Result<(), CodecError> {
        let mut buffer = vec![0u8; 7200];
        let size = c_int::try_from(buffer.len()).unwrap_or(c_int::MAX);
        // SAFETY: a live encoder and a buffer of `size` writable bytes.
        let code = unsafe { (self.api().flush)(self.handle.as_ptr(), buffer.as_mut_ptr(), size) };
        append_encoded(&buffer, code, out)
    }
}

/// Which `id3tag_set_*` function [`Lame::id3tag_set`] calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Id3Field {
    Title,
    Artist,
    Album,
    Year,
    Comment,
    Track,
    Genre,
}

/// An output buffer of LAME's worst case for `frames` frames: `1.25 * frames + 7200` bytes.
fn encode_buffer(frames: usize) -> Vec<u8> {
    vec![0u8; frames.saturating_mul(5) / 4 + 7200]
}

/// Appends the first `code` bytes of `buffer` to `out`, or reports a negative `code` (or one
/// beyond the buffer).
fn append_encoded(buffer: &[u8], code: c_int, out: &mut Vec<u8>) -> Result<(), CodecError> {
    let encoded = usize::try_from(code)
        .ok()
        .and_then(|bytes| buffer.get(..bytes))
        .ok_or_else(|| Lame::error("lame_encode", format!("returned {code}")))?;
    out.extend_from_slice(encoded);
    Ok(())
}

impl Drop for Lame {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `lame_init` and is closed exactly once, here.
        unsafe { (self.library.0.close)(self.handle.as_ptr()) };
    }
}

// SAFETY: a LAME encoder has no thread affinity; the handle is owned exclusively by this value
// and every mutating call goes through `&mut self`.
unsafe impl Send for Lame {}

// ---------------------------------------------------------------------------------------------
// libFLAC
// ---------------------------------------------------------------------------------------------

/// `FLAC__StreamEncoder *`.
type FlacEncoderHandle = *mut c_void;
/// `FLAC__StreamMetadata *`.
type FlacMetadataHandle = *mut c_void;
/// `FLAC__bool`, an `int`.
type FlacBool = c_int;

/// `FLAC__METADATA_TYPE_VORBIS_COMMENT` of the `FLAC__MetadataType` enum.
const FLAC_METADATA_TYPE_VORBIS_COMMENT: c_int = 4;
/// `FLAC__STREAM_ENCODER_INIT_STATUS_OK`.
const FLAC_INIT_STATUS_OK: c_int = 0;
/// `FLAC__STREAM_ENCODER_WRITE_STATUS_OK` and `…_FATAL_ERROR`.
const FLAC_WRITE_STATUS_OK: c_int = 0;
const FLAC_WRITE_STATUS_FATAL_ERROR: c_int = 1;
/// `FLAC__STREAM_ENCODER_SEEK_STATUS_OK` and `…_ERROR`.
const FLAC_SEEK_STATUS_OK: c_int = 0;
const FLAC_SEEK_STATUS_ERROR: c_int = 1;
/// `FLAC__STREAM_ENCODER_TELL_STATUS_OK` and `…_ERROR`.
const FLAC_TELL_STATUS_OK: c_int = 0;
const FLAC_TELL_STATUS_ERROR: c_int = 1;

/// The names of `FLAC__StreamEncoderInitStatus`, in the enum's order.
const FLAC_INIT_STATUS_NAMES: [&str; 14] = [
    "FLAC__STREAM_ENCODER_INIT_STATUS_OK",
    "FLAC__STREAM_ENCODER_INIT_STATUS_ENCODER_ERROR",
    "FLAC__STREAM_ENCODER_INIT_STATUS_UNSUPPORTED_CONTAINER",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_CALLBACKS",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_NUMBER_OF_CHANNELS",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_BITS_PER_SAMPLE",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_SAMPLE_RATE",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_BLOCK_SIZE",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_MAX_LPC_ORDER",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_QLP_COEFF_PRECISION",
    "FLAC__STREAM_ENCODER_INIT_STATUS_BLOCK_SIZE_TOO_SMALL_FOR_LPC_ORDER",
    "FLAC__STREAM_ENCODER_INIT_STATUS_NOT_STREAMABLE",
    "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_METADATA",
    "FLAC__STREAM_ENCODER_INIT_STATUS_ALREADY_INITIALIZED",
];

/// `FLAC__StreamMetadata_VorbisComment_Entry`.
#[repr(C)]
#[derive(Clone, Copy)]
struct FlacCommentEntry {
    length: u32,
    entry: *mut u8,
}

type FlacWriteCallback =
    unsafe extern "C" fn(*const c_void, *const u8, usize, u32, u32, *mut c_void) -> c_int;
type FlacSeekCallback = unsafe extern "C" fn(*const c_void, u64, *mut c_void) -> c_int;
type FlacTellCallback = unsafe extern "C" fn(*const c_void, *mut u64, *mut c_void) -> c_int;
type FlacMetadataCallback = unsafe extern "C" fn(*const c_void, *const c_void, *mut c_void);

type FlacEncoderNewFn = unsafe extern "C" fn() -> FlacEncoderHandle;
type FlacEncoderDeleteFn = unsafe extern "C" fn(FlacEncoderHandle);
type FlacEncoderSetU32Fn = unsafe extern "C" fn(FlacEncoderHandle, u32) -> FlacBool;
type FlacEncoderSetMetadataFn =
    unsafe extern "C" fn(FlacEncoderHandle, *mut FlacMetadataHandle, u32) -> FlacBool;
type FlacEncoderInitStreamFn = unsafe extern "C" fn(
    FlacEncoderHandle,
    Option<FlacWriteCallback>,
    Option<FlacSeekCallback>,
    Option<FlacTellCallback>,
    Option<FlacMetadataCallback>,
    *mut c_void,
) -> c_int;
type FlacEncoderProcessInterleavedFn =
    unsafe extern "C" fn(FlacEncoderHandle, *const i32, u32) -> FlacBool;
type FlacEncoderFinishFn = unsafe extern "C" fn(FlacEncoderHandle) -> FlacBool;
type FlacEncoderStateStringFn = unsafe extern "C" fn(*const c_void) -> *const c_char;
type FlacMetadataNewFn = unsafe extern "C" fn(c_int) -> FlacMetadataHandle;
type FlacMetadataDeleteFn = unsafe extern "C" fn(FlacMetadataHandle);
type FlacEntryFromPairFn =
    unsafe extern "C" fn(*mut FlacCommentEntry, *const c_char, *const c_char) -> FlacBool;
type FlacAppendCommentFn =
    unsafe extern "C" fn(FlacMetadataHandle, FlacCommentEntry, FlacBool) -> FlacBool;

/// The resolved libFLAC functions. The function pointers are valid while `_library` is loaded;
/// every user holds the table through an `Arc`, so they cannot outlive it.
struct FlacApi {
    encoder_new: FlacEncoderNewFn,
    encoder_delete: FlacEncoderDeleteFn,
    set_channels: FlacEncoderSetU32Fn,
    set_bits_per_sample: FlacEncoderSetU32Fn,
    set_sample_rate: FlacEncoderSetU32Fn,
    set_compression_level: FlacEncoderSetU32Fn,
    set_metadata: FlacEncoderSetMetadataFn,
    init_stream: FlacEncoderInitStreamFn,
    process_interleaved: FlacEncoderProcessInterleavedFn,
    finish: FlacEncoderFinishFn,
    state_string: FlacEncoderStateStringFn,
    metadata_new: FlacMetadataNewFn,
    metadata_delete: FlacMetadataDeleteFn,
    entry_from_name_value_pair: FlacEntryFromPairFn,
    append_comment: FlacAppendCommentFn,
    _library: Library,
}

impl FlacApi {
    /// Resolves every function the wrapper uses.
    fn resolve(library: Library, path: &str) -> Result<Self, CodecError> {
        let n = FLAC.name;
        // SAFETY: every signature below is the one declared in libFLAC 1.4.3's
        // `FLAC/stream_encoder.h`, `FLAC/metadata.h` and `FLAC/format.h` (`FLAC__bool` and the
        // enums are `int`, `uint32_t` is `u32`, `FLAC__uint64` is `u64`, the encoder and
        // metadata types are opaque pointers here); 1.5.0 declares the same. The pointers live
        // in the same `FlacApi` as `library`, which keeps the library loaded while they can be
        // called.
        unsafe {
            Ok(Self {
                encoder_new: symbol(&library, n, path, "FLAC__stream_encoder_new")?,
                encoder_delete: symbol(&library, n, path, "FLAC__stream_encoder_delete")?,
                set_channels: symbol(&library, n, path, "FLAC__stream_encoder_set_channels")?,
                set_bits_per_sample: symbol(
                    &library,
                    n,
                    path,
                    "FLAC__stream_encoder_set_bits_per_sample",
                )?,
                set_sample_rate: symbol(&library, n, path, "FLAC__stream_encoder_set_sample_rate")?,
                set_compression_level: symbol(
                    &library,
                    n,
                    path,
                    "FLAC__stream_encoder_set_compression_level",
                )?,
                set_metadata: symbol(&library, n, path, "FLAC__stream_encoder_set_metadata")?,
                init_stream: symbol(&library, n, path, "FLAC__stream_encoder_init_stream")?,
                process_interleaved: symbol(
                    &library,
                    n,
                    path,
                    "FLAC__stream_encoder_process_interleaved",
                )?,
                finish: symbol(&library, n, path, "FLAC__stream_encoder_finish")?,
                state_string: symbol(
                    &library,
                    n,
                    path,
                    "FLAC__stream_encoder_get_resolved_state_string",
                )?,
                metadata_new: symbol(&library, n, path, "FLAC__metadata_object_new")?,
                metadata_delete: symbol(&library, n, path, "FLAC__metadata_object_delete")?,
                entry_from_name_value_pair: symbol(
                    &library,
                    n,
                    path,
                    "FLAC__metadata_object_vorbiscomment_entry_from_name_value_pair",
                )?,
                append_comment: symbol(
                    &library,
                    n,
                    path,
                    "FLAC__metadata_object_vorbiscomment_append_comment",
                )?,
                _library: library,
            })
        }
    }
}

/// A loaded libFLAC, shared by every encoder.
#[derive(Clone)]
pub(crate) struct FlacLibrary(Arc<FlacApi>);

impl FlacLibrary {
    /// The process-wide libFLAC, loaded on first use from the candidate locations.
    ///
    /// # Errors
    ///
    /// [`CodecError::LibraryNotFound`] when no candidate loads (the result is remembered), or
    /// [`CodecError::MissingSymbol`] when the loaded file is not a compatible libFLAC.
    pub(crate) fn shared() -> Result<Self, CodecError> {
        static LIBRARY: OnceLock<Result<FlacLibrary, CodecError>> = OnceLock::new();
        LIBRARY
            .get_or_init(|| Self::load_first(candidates_from_env(&FLAC)))
            .clone()
    }

    /// Loads the first candidate that is a loadable libFLAC.
    fn load_first(candidates: Vec<OsString>) -> Result<Self, CodecError> {
        load_first(FLAC.name, candidates, FlacApi::resolve).map(|api| Self(Arc::new(api)))
    }
}

/// A libFLAC encoder call failed: libFLAC reported it, or the output its callbacks write to
/// failed first.
#[derive(Debug)]
pub(crate) enum FlacEncodeError {
    /// libFLAC refused the call; the message names the encoder state or init status.
    Codec(CodecError),
    /// Writing to or seeking the output failed (the first such error, ERR-04).
    Io(io::Error),
}

impl From<CodecError> for FlacEncodeError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// What an encoder's callbacks write to: the output, the byte offset in it, and the first I/O
/// error. libFLAC asks for the offset before every frame, so it is counted here instead of
/// asking the output, which for a `BufWriter` would flush it each time.
struct FlacSink<W> {
    /// The output; taken out by [`FlacEncoder::finish`].
    output: Option<W>,
    position: u64,
    error: Option<io::Error>,
}

/// Records the first I/O error of a callback.
fn note_io_error<W>(sink: &mut FlacSink<W>, error: io::Error) {
    if sink.error.is_none() {
        sink.error = Some(error);
    }
}

/// `FLAC__StreamEncoderWriteCallback`: appends `bytes` bytes to the output.
///
/// # Safety
///
/// `client_data` is the `FlacSink<W>` of the [`FlacEncoder<W>`] that passed it to
/// `FLAC__stream_encoder_init_stream`, and no other reference to it exists during the call;
/// `buffer` points to `bytes` readable bytes. libFLAC guarantees the buffer; [`FlacEncoder`]
/// guarantees the sink.
unsafe extern "C" fn flac_write<W: Write + Seek>(
    _encoder: *const c_void,
    buffer: *const u8,
    bytes: usize,
    _samples: u32,
    _current_frame: u32,
    client_data: *mut c_void,
) -> c_int {
    // SAFETY: the caller's contract: `client_data` is the live sink, not otherwise borrowed.
    let sink = unsafe { &mut *client_data.cast::<FlacSink<W>>() };
    if bytes == 0 {
        return FLAC_WRITE_STATUS_OK;
    }
    if buffer.is_null() {
        note_io_error(sink, io::Error::other("libFLAC passed no buffer"));
        return FLAC_WRITE_STATUS_FATAL_ERROR;
    }
    // SAFETY: libFLAC passes `bytes` readable bytes at the non-null `buffer`, valid during the
    // call.
    let data = unsafe { std::slice::from_raw_parts(buffer, bytes) };
    let Some(output) = sink.output.as_mut() else {
        return FLAC_WRITE_STATUS_FATAL_ERROR;
    };
    match output.write_all(data) {
        Ok(()) => {
            let written = u64::try_from(bytes).unwrap_or(u64::MAX);
            sink.position = sink.position.saturating_add(written);
            FLAC_WRITE_STATUS_OK
        }
        Err(error) => {
            note_io_error(sink, error);
            FLAC_WRITE_STATUS_FATAL_ERROR
        }
    }
}

/// `FLAC__StreamEncoderSeekCallback`: moves to `offset` bytes from the start of the output.
///
/// # Safety
///
/// As for [`flac_write`]: `client_data` is the encoder's live `FlacSink<W>`, not otherwise
/// borrowed during the call.
unsafe extern "C" fn flac_seek<W: Write + Seek>(
    _encoder: *const c_void,
    offset: u64,
    client_data: *mut c_void,
) -> c_int {
    // SAFETY: the caller's contract: `client_data` is the live sink, not otherwise borrowed.
    let sink = unsafe { &mut *client_data.cast::<FlacSink<W>>() };
    let Some(output) = sink.output.as_mut() else {
        return FLAC_SEEK_STATUS_ERROR;
    };
    // `BufWriter::seek` writes out its buffer first.
    match output.seek(io::SeekFrom::Start(offset)) {
        Ok(position) => {
            sink.position = position;
            FLAC_SEEK_STATUS_OK
        }
        Err(error) => {
            note_io_error(sink, error);
            FLAC_SEEK_STATUS_ERROR
        }
    }
}

/// `FLAC__StreamEncoderTellCallback`: the current byte offset in the output.
///
/// # Safety
///
/// As for [`flac_write`]; `offset` is null or a writable `u64`.
unsafe extern "C" fn flac_tell<W: Write + Seek>(
    _encoder: *const c_void,
    offset: *mut u64,
    client_data: *mut c_void,
) -> c_int {
    // SAFETY: the caller's contract: `client_data` is the live sink, not otherwise borrowed.
    let sink = unsafe { &*client_data.cast::<FlacSink<W>>() };
    if offset.is_null() {
        return FLAC_TELL_STATUS_ERROR;
    }
    // SAFETY: libFLAC passes a valid, writable out-pointer (checked for null above).
    unsafe { offset.write(sink.position) };
    FLAC_TELL_STATUS_OK
}

/// One libFLAC stream encoder (`FLAC__StreamEncoder`) writing a native FLAC stream to `W`
/// through write, seek and tell callbacks, as libsndfile's `flac.c` sets it up. The methods are
/// libFLAC's setters and calls with checked arguments; dropping it deletes the encoder, then
/// the comment block, then the output (MEM-07).
///
/// **Threads.** Each encoder runs on the thread that calls it, and libFLAC 1.4.3 keeps no
/// mutable global state: the CPU detection and the choice of the intrinsic routines fill each
/// encoder's own structures (`FLAC__stream_encoder_init_stream`), the apodization windows are
/// computed into each encoder's own buffers, the CRC and log2 tables are `const`, and the 1.4
/// encoder starts no threads (worker threads only arrive with 1.5's
/// `FLAC__stream_encoder_set_num_threads`, which is never called, so 1.5 stays single-threaded
/// too). Unlike libsoxr (see `SOXR_LOCK`), separate encoders therefore run on separate worker
/// threads without a lock.
///
/// `W`'s `write` and `seek` run inside libFLAC's callbacks, where a panic would abort the
/// process; the file and memory outputs used here return errors instead.
pub(crate) struct FlacEncoder<W: Write + Seek> {
    library: FlacLibrary,
    handle: NonNull<c_void>,
    /// The `VORBIS_COMMENT` object handed to the encoder, deleted after it.
    metadata: Option<NonNull<c_void>>,
    /// The callbacks' sink, from `Box::leak`: it stays at this address, as libFLAC's client
    /// data, until `Drop` frees it after deleting the encoder.
    sink: NonNull<FlacSink<W>>,
    /// The channel count set on the encoder; needed before the first samples.
    channels: Option<u32>,
}

/// A `CodecError::Call` from libFLAC.
fn flac_error(operation: &'static str, message: impl Into<String>) -> CodecError {
    CodecError::Call {
        library: FLAC.name,
        operation,
        message: message.into(),
    }
}

impl<W: Write + Seek> FlacEncoder<W> {
    /// `FLAC__stream_encoder_new()`, writing to `output` from its current position once
    /// initialized.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] when libFLAC cannot allocate an encoder.
    pub(crate) fn new(library: FlacLibrary, output: W) -> Result<Self, CodecError> {
        // SAFETY: `FLAC__stream_encoder_new` takes no arguments and returns a new encoder or
        // null.
        let handle = unsafe { (library.0.encoder_new)() };
        let handle = NonNull::new(handle)
            .ok_or_else(|| flac_error("FLAC__stream_encoder_new", "no encoder"))?;
        let sink = NonNull::from(Box::leak(Box::new(FlacSink {
            output: Some(output),
            position: 0,
            error: None,
        })));
        Ok(Self {
            library,
            handle,
            metadata: None,
            sink,
            channels: None,
        })
    }

    fn api(&self) -> &FlacApi {
        &self.library.0
    }

    /// Calls one of the `FLAC__stream_encoder_set_*` functions that take a `uint32_t`; they
    /// return false only for an encoder that is already initialized.
    fn set(
        &mut self,
        operation: &'static str,
        setter: FlacEncoderSetU32Fn,
        value: u32,
    ) -> Result<(), CodecError> {
        // SAFETY: a live encoder and a plain value; libFLAC only stores it (validation happens
        // in `FLAC__stream_encoder_init_stream`).
        if unsafe { setter(self.handle.as_ptr(), value) } == 0 {
            Err(flac_error(operation, format!("{value} rejected")))
        } else {
            Ok(())
        }
    }

    /// `FLAC__stream_encoder_set_channels`.
    pub(crate) fn set_channels(&mut self, channels: u32) -> Result<(), CodecError> {
        let setter = self.api().set_channels;
        self.set("FLAC__stream_encoder_set_channels", setter, channels)?;
        self.channels = Some(channels);
        Ok(())
    }

    /// `FLAC__stream_encoder_set_sample_rate`.
    pub(crate) fn set_sample_rate(&mut self, rate: u32) -> Result<(), CodecError> {
        let setter = self.api().set_sample_rate;
        self.set("FLAC__stream_encoder_set_sample_rate", setter, rate)
    }

    /// `FLAC__stream_encoder_set_bits_per_sample`.
    pub(crate) fn set_bits_per_sample(&mut self, bits: u32) -> Result<(), CodecError> {
        let setter = self.api().set_bits_per_sample;
        self.set("FLAC__stream_encoder_set_bits_per_sample", setter, bits)
    }

    /// `FLAC__stream_encoder_set_compression_level` (0 to 8).
    pub(crate) fn set_compression_level(&mut self, level: u32) -> Result<(), CodecError> {
        let setter = self.api().set_compression_level;
        self.set("FLAC__stream_encoder_set_compression_level", setter, level)
    }

    /// Hands the encoder one `VORBIS_COMMENT` block with a `name=value` entry per field, in
    /// order, as libsndfile's `flac_write_strings` builds it: `FLAC__metadata_object_new`, then
    /// per field `FLAC__metadata_object_vorbiscomment_entry_from_name_value_pair` and
    /// `FLAC__metadata_object_vorbiscomment_append_comment` without a copy, then
    /// `FLAC__stream_encoder_set_metadata` with the one block. A value is passed up to its first
    /// NUL, as a C string. libFLAC writes its own vendor string into the block.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] when the block is already set, libFLAC cannot allocate, or it
    /// rejects a field: a name outside the Vorbis rules or a value that is not UTF-8.
    /// libsndfile ignores that last result and appends an uninitialized entry (undefined
    /// behaviour); this wrapper reports it instead (ERR-06).
    pub(crate) fn set_vorbis_comments(
        &mut self,
        fields: &[(&str, &[u8])],
    ) -> Result<(), CodecError> {
        if self.metadata.is_some() {
            return Err(flac_error(
                "FLAC__stream_encoder_set_metadata",
                "the comment block is already set",
            ));
        }
        let api = Arc::clone(&self.library.0);
        // SAFETY: a plain enum value; returns a new object (with libFLAC's vendor string and no
        // comments) or null.
        let object = unsafe { (api.metadata_new)(FLAC_METADATA_TYPE_VORBIS_COMMENT) };
        let object = NonNull::new(object)
            .ok_or_else(|| flac_error("FLAC__metadata_object_new", "no metadata object"))?;
        // Owned from here: `Drop` deletes it after the encoder.
        self.metadata = Some(object);
        for (name, value) in fields {
            let pair_error = |message: String| {
                flac_error(
                    "FLAC__metadata_object_vorbiscomment_entry_from_name_value_pair",
                    message,
                )
            };
            let c_name = CString::new(*name)
                .map_err(|_| pair_error(format!("the name {name:?} contains NUL")))?;
            let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
            let c_value = CString::new(value.get(..end).unwrap_or_default())
                .map_err(|_| pair_error(format!("the {name} value contains NUL")))?;
            let mut entry = FlacCommentEntry {
                length: 0,
                entry: ptr::null_mut(),
            };
            // SAFETY: `entry` is a valid out-pointer, and both strings are NUL-terminated and
            // outlive the call. libFLAC checks the name and that the value is UTF-8, and only
            // then stores a `malloc`ed `name=value` in `entry`.
            let made = unsafe {
                (api.entry_from_name_value_pair)(&raw mut entry, c_name.as_ptr(), c_value.as_ptr())
            };
            if made == 0 {
                return Err(pair_error(format!(
                    "the {name} comment is not a legal Vorbis comment (the value must be UTF-8)"
                )));
            }
            // SAFETY: `object` is a live VORBIS_COMMENT object and `entry` the entry libFLAC just
            // made. Without a copy (`copy = false`) the object takes ownership of `entry.entry`
            // and frees it with the object.
            if unsafe { (api.append_comment)(object.as_ptr(), entry, 0) } == 0 {
                // libFLAC refuses only when it cannot grow the comment array (the entry is
                // legal); the entry then stays unowned and leaks, since only libFLAC's
                // allocator can free it.
                return Err(flac_error(
                    "FLAC__metadata_object_vorbiscomment_append_comment",
                    format!("cannot append the {name} comment"),
                ));
            }
        }
        let mut blocks = [object.as_ptr()];
        // SAFETY: a live, uninitialized encoder and an array of one live metadata object.
        // libFLAC copies the array; the object stays alive until `Drop` deletes it after the
        // encoder.
        if unsafe { (api.set_metadata)(self.handle.as_ptr(), blocks.as_mut_ptr(), 1) } == 0 {
            return Err(flac_error(
                "FLAC__stream_encoder_set_metadata",
                "the encoder is already initialized",
            ));
        }
        Ok(())
    }

    /// `FLAC__stream_encoder_get_resolved_state_string`: the encoder state, or the verify
    /// decoder's state when that is the cause.
    fn state(&self) -> String {
        // SAFETY: a live encoder; the result is a static, NUL-terminated name.
        unsafe { c_text((self.api().state_string)(self.handle.as_ptr())) }
            .unwrap_or_else(|| "unknown encoder state".to_owned())
    }

    /// The error for a failed call: the output's I/O error when a callback hit one, else
    /// libFLAC's `message`.
    fn failure(&mut self, operation: &'static str, message: String) -> FlacEncodeError {
        // SAFETY: the libFLAC call has returned, so this is the only reference to the sink.
        match unsafe { self.sink.as_mut() }.error.take() {
            Some(error) => FlacEncodeError::Io(error),
            None => FlacEncodeError::Codec(flac_error(operation, message)),
        }
    }

    /// `FLAC__stream_encoder_init_stream` with the write, seek and tell callbacks and no
    /// metadata callback: writes `fLaC`, `STREAMINFO` and the metadata blocks.
    ///
    /// # Errors
    ///
    /// [`FlacEncodeError::Io`] when the output fails, else [`FlacEncodeError::Codec`] with the
    /// init status, for example `FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_SAMPLE_RATE`, or when
    /// no channel count was set.
    pub(crate) fn init_stream(&mut self) -> Result<(), FlacEncodeError> {
        if self.channels.is_none() {
            return Err(flac_error(
                "FLAC__stream_encoder_init_stream",
                "the channel count is not set",
            )
            .into());
        }
        {
            // SAFETY: no libFLAC call is running, so this is the only reference to the sink.
            let sink = unsafe { self.sink.as_mut() };
            let Some(output) = sink.output.as_mut() else {
                return Err(flac_error("FLAC__stream_encoder_init_stream", "no output").into());
            };
            sink.position = output.stream_position().map_err(FlacEncodeError::Io)?;
        }
        // SAFETY: a live encoder; the callbacks are this module's functions for `W`, and the
        // client data is the `FlacSink<W>` they expect. It stays at its address until `Drop`
        // frees it after deleting the encoder, and libFLAC calls back only during
        // `init_stream`, `process_interleaved` and `finish` (deleting runs no callbacks), while
        // this wrapper holds no other reference to the sink.
        let status = unsafe {
            (self.api().init_stream)(
                self.handle.as_ptr(),
                Some(flac_write::<W>),
                Some(flac_seek::<W>),
                Some(flac_tell::<W>),
                None,
                self.sink.as_ptr().cast::<c_void>(),
            )
        };
        if status == FLAC_INIT_STATUS_OK {
            return Ok(());
        }
        let name = usize::try_from(status)
            .ok()
            .and_then(|i| FLAC_INIT_STATUS_NAMES.get(i))
            .map_or_else(
                || format!("init status {status}"),
                |name| (*name).to_owned(),
            );
        let message = format!("{name} ({})", self.state());
        Err(self.failure("FLAC__stream_encoder_init_stream", message))
    }

    /// `FLAC__stream_encoder_process_interleaved` with interleaved samples of the configured
    /// channel count, each within the bit depth.
    ///
    /// # Errors
    ///
    /// [`FlacEncodeError::Io`] when the output fails, else [`FlacEncodeError::Codec`] with the
    /// encoder state: not initialized, a sample out of range (`FLAC__STREAM_ENCODER_CLIENT_ERROR`)
    /// or a sample count that is not whole frames.
    pub(crate) fn process_interleaved(&mut self, samples: &[i32]) -> Result<(), FlacEncodeError> {
        let operation = "FLAC__stream_encoder_process_interleaved";
        let channels = self
            .channels
            .and_then(|c| usize::try_from(c).ok())
            .filter(|&c| c > 0)
            .ok_or_else(|| flac_error(operation, "the channel count is not set"))?;
        if !samples.len().is_multiple_of(channels) {
            return Err(flac_error(
                operation,
                format!(
                    "{} samples are not whole frames of {channels}",
                    samples.len()
                ),
            )
            .into());
        }
        let frames = u32::try_from(samples.len() / channels)
            .map_err(|_| flac_error(operation, "too many samples"))?;
        // SAFETY: a live encoder configured for `channels` channels; `samples` holds
        // `frames * channels` readable values (checked above), which libFLAC only reads during
        // the call. An uninitialized encoder returns false without reading. The callbacks may
        // run (see `init_stream`).
        let accepted = unsafe {
            (self.api().process_interleaved)(self.handle.as_ptr(), samples.as_ptr(), frames)
        };
        if accepted == 0 {
            let state = self.state();
            Err(self.failure(operation, state))
        } else {
            Ok(())
        }
    }

    /// `FLAC__stream_encoder_finish`: encodes the last partial block and, through the seek
    /// callback, rewrites `STREAMINFO`'s frame sizes, total samples and MD5. Returns the output
    /// (not flushed).
    ///
    /// # Errors
    ///
    /// [`FlacEncodeError::Io`] when the output fails, else [`FlacEncodeError::Codec`] with the
    /// encoder state.
    pub(crate) fn finish(mut self) -> Result<W, FlacEncodeError> {
        // SAFETY: a live encoder; the callbacks may run (see `init_stream`). An uninitialized
        // encoder returns true without writing.
        let finished = unsafe { (self.api().finish)(self.handle.as_ptr()) };
        if finished == 0 {
            let state = self.state();
            return Err(self.failure("FLAC__stream_encoder_finish", state));
        }
        // SAFETY: the encoder is finished, so no callback runs any more; this is the only
        // reference to the sink.
        let sink = unsafe { self.sink.as_mut() };
        if let Some(error) = sink.error.take() {
            return Err(FlacEncodeError::Io(error));
        }
        sink.output
            .take()
            .ok_or_else(|| flac_error("FLAC__stream_encoder_finish", "no output").into())
    }
}

impl<W: Write + Seek> Drop for FlacEncoder<W> {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `FLAC__stream_encoder_new` and is deleted exactly once,
        // here. Deleting an encoder that is still initialized runs no callbacks (libFLAC marks
        // it as being deleted and skips the last frame and the metadata update).
        unsafe { (self.library.0.encoder_delete)(self.handle.as_ptr()) };
        if let Some(object) = self.metadata.take() {
            // SAFETY: the object came from `FLAC__metadata_object_new`, the encoder that
            // pointed to it is deleted, and it is deleted exactly once, here, with the entries
            // it owns.
            unsafe { (self.library.0.metadata_delete)(object.as_ptr()) };
        }
        // SAFETY: `sink` came from `Box::leak` in `new` and is freed exactly once, here, after
        // the encoder that called back into it is deleted.
        drop(unsafe { Box::from_raw(self.sink.as_ptr()) });
    }
}

// ---------------------------------------------------------------------------------------------
// libopus: the multistream decoder
// ---------------------------------------------------------------------------------------------

/// The libopus multistream decoder (`opus_multistream_decoder_*`), the decoder libsndfile's Ogg
/// Opus reader uses for every channel mapping (`ogg_opus.c`). libopus is BSD-licensed and
/// statically linked through `opusic-sys`, so nothing is loaded at run time here; the `opus`
/// crate wraps only the mono and stereo decoder, which cannot decode mapping family 1 (QG-13).
pub(crate) struct OpusMsDecoder {
    handle: NonNull<opusic_sys::OpusMSDecoder>,
    channels: usize,
}

/// The longest Opus packet, 120 ms at 48 kHz, in frames per channel.
pub(crate) const OPUS_MAX_FRAMES: usize = 5760;

fn opus_error(operation: &'static str, code: c_int) -> CodecError {
    // SAFETY: `opus_strerror` returns a static, NUL-terminated string for every code.
    let message = unsafe { c_text(opusic_sys::opus_strerror(code)) }
        .unwrap_or_else(|| format!("error {code}"));
    CodecError::Call {
        library: "libopus",
        operation,
        message,
    }
}

impl OpusMsDecoder {
    /// A decoder at `sample_rate` (8, 12, 16, 24 or 48 kHz) for `mapping.len()` channels from
    /// `streams` streams, of which `coupled` are stereo, with the `OpusHead` output gain
    /// `gain_q8` (dB in Q7.8) applied, as libsndfile sets it (`OPUS_SET_GAIN`).
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] when libopus rejects the layout or the rate.
    pub(crate) fn new(
        sample_rate: u32,
        streams: u8,
        coupled: u8,
        mapping: &[u8],
        gain_q8: i16,
    ) -> Result<Self, CodecError> {
        let channels = c_int::try_from(mapping.len())
            .map_err(|_| opus_error("opus_multistream_decoder_create", opusic_sys::OPUS_BAD_ARG))?;
        let rate = i32::try_from(sample_rate)
            .map_err(|_| opus_error("opus_multistream_decoder_create", opusic_sys::OPUS_BAD_ARG))?;
        let mut error: c_int = 0;
        // SAFETY: `mapping` holds `channels` bytes and outlives the call (libopus copies it);
        // `error` is a valid out pointer.
        let raw = unsafe {
            opusic_sys::opus_multistream_decoder_create(
                rate,
                channels,
                c_int::from(streams),
                c_int::from(coupled),
                mapping.as_ptr(),
                &raw mut error,
            )
        };
        let handle = match NonNull::new(raw) {
            Some(handle) if error == opusic_sys::OPUS_OK => handle,
            Some(handle) => {
                // SAFETY: the decoder was created and is destroyed exactly once, here.
                unsafe { opusic_sys::opus_multistream_decoder_destroy(handle.as_ptr()) };
                return Err(opus_error("opus_multistream_decoder_create", error));
            }
            None => return Err(opus_error("opus_multistream_decoder_create", error)),
        };
        let decoder = Self {
            handle,
            channels: mapping.len(),
        };
        // SAFETY: a live decoder; `OPUS_SET_GAIN` takes one `opus_int32` argument.
        let status = unsafe {
            opusic_sys::opus_multistream_decoder_ctl(
                decoder.handle.as_ptr(),
                opusic_sys::OPUS_SET_GAIN_REQUEST,
                i32::from(gain_q8),
            )
        };
        if status != opusic_sys::OPUS_OK {
            return Err(opus_error("OPUS_SET_GAIN", status));
        }
        Ok(decoder)
    }

    /// Decodes one packet into `out` (interleaved, at most [`OPUS_MAX_FRAMES`] frames, so
    /// `out` holds `OPUS_MAX_FRAMES * channels` values) and returns the frames decoded.
    ///
    /// # Errors
    ///
    /// [`CodecError::Call`] for a packet libopus cannot decode.
    pub(crate) fn decode_float(
        &mut self,
        packet: &[u8],
        out: &mut [f32],
    ) -> Result<usize, CodecError> {
        let operation = "opus_multistream_decode_float";
        let capacity = out.len() / self.channels.max(1);
        let frame_size = c_int::try_from(capacity.min(OPUS_MAX_FRAMES))
            .map_err(|_| opus_error(operation, opusic_sys::OPUS_BAD_ARG))?;
        let len = i32::try_from(packet.len())
            .map_err(|_| opus_error(operation, opusic_sys::OPUS_INVALID_PACKET))?;
        // SAFETY: a live decoder; `packet` is readable for `len` bytes, and `out` is writable
        // for `frame_size * channels` floats (`frame_size <= out.len() / channels`).
        let decoded = unsafe {
            opusic_sys::opus_multistream_decode_float(
                self.handle.as_ptr(),
                packet.as_ptr(),
                len,
                out.as_mut_ptr(),
                frame_size,
                0,
            )
        };
        usize::try_from(decoded).map_err(|_| opus_error(operation, decoded))
    }
}

impl Drop for OpusMsDecoder {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `opus_multistream_decoder_create` and is destroyed exactly
        // once, here.
        unsafe { opusic_sys::opus_multistream_decoder_destroy(self.handle.as_ptr()) };
    }
}

// SAFETY: an Opus decoder has no thread affinity; the handle is owned exclusively by this value
// and every call goes through `&mut self`.
unsafe impl Send for OpusMsDecoder {}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::{self, Cursor, Seek, SeekFrom, Write};
    use std::path::Path;

    use super::{
        CodecError, FLAC, FlacEncodeError, FlacEncoder, FlacLibrary, LAME, LameLibrary, SOXR,
        SoxrLibrary, candidates,
    };

    #[test]
    fn the_native_directory_comes_after_the_bundle_and_before_the_system() {
        for spec in [&SOXR, &LAME, &FLAC] {
            let found = candidates(spec, Some(Path::new("/opt/sonara-native")));
            let native = Path::new("/opt/sonara-native").join(spec.file);
            let system = OsString::from(spec.file);
            if cfg!(target_os = "macos") {
                assert_eq!(found.len(), 3, "{found:?}");
                let bundle = Path::new(&found[0]);
                assert!(bundle.ends_with(Path::new("../Frameworks").join(spec.file)));
            } else {
                assert_eq!(found.len(), 2, "{found:?}");
            }
            assert_eq!(found[found.len() - 2], native.into_os_string());
            assert_eq!(found[found.len() - 1], system);
        }
    }

    #[test]
    fn an_unset_or_empty_native_directory_is_not_searched() {
        for native_dir in [None, Some(Path::new(""))] {
            let found = candidates(&SOXR, native_dir);
            assert_eq!(found.len(), if cfg!(target_os = "macos") { 2 } else { 1 });
            assert_eq!(found.last(), Some(&OsString::from(SOXR.file)));
        }
    }

    #[test]
    fn a_missing_library_is_a_typed_error_listing_the_candidates() {
        let err = SoxrLibrary::load_first(vec![
            OsString::from("/nonexistent/libsoxr.so.0"),
            OsString::from("libsoxr-does-not-exist.so"),
        ])
        .err()
        .expect("nothing loads");
        match &err {
            CodecError::LibraryNotFound { library, tried } => {
                assert_eq!(*library, "libsoxr");
                assert_eq!(tried.len(), 2);
                assert!(tried[0].starts_with("/nonexistent/libsoxr.so.0: "));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(
            err.to_string()
                .starts_with("libsoxr is not available; tried ")
        );
    }

    #[test]
    fn a_missing_lame_is_a_typed_error() {
        let err = LameLibrary::load_first(vec![OsString::from("/nonexistent/libmp3lame.so.0")])
            .err()
            .expect("nothing loads");
        assert!(
            matches!(&err, CodecError::LibraryNotFound { library: "libmp3lame", tried } if tried.len() == 1),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("libmp3lame is not available; tried /nonexistent/libmp3lame.so.0: ")
        );
    }

    #[test]
    fn a_missing_libflac_is_a_typed_error() {
        let err = FlacLibrary::load_first(vec![OsString::from("/nonexistent/libFLAC.12.dylib")])
            .err()
            .expect("nothing loads");
        assert!(
            matches!(&err, CodecError::LibraryNotFound { library: "libFLAC", tried } if tried.len() == 1),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("libFLAC is not available; tried /nonexistent/libFLAC.12.dylib: ")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_library_without_the_api_is_rejected() {
        let err = SoxrLibrary::load_first(vec![OsString::from("libc.so.6")])
            .err()
            .expect("libc is not libsoxr");
        assert!(
            matches!(
                err,
                CodecError::MissingSymbol {
                    symbol: "soxr_version",
                    ..
                }
            ),
            "{err:?}"
        );
        let err = LameLibrary::load_first(vec![OsString::from("libc.so.6")])
            .err()
            .expect("libc is not LAME");
        assert!(
            matches!(
                err,
                CodecError::MissingSymbol {
                    library: "libmp3lame",
                    symbol: "lame_init",
                    ..
                }
            ),
            "{err:?}"
        );
        let err = FlacLibrary::load_first(vec![OsString::from("libc.so.6")])
            .err()
            .expect("libc is not libFLAC");
        assert!(
            matches!(
                err,
                CodecError::MissingSymbol {
                    library: "libFLAC",
                    symbol: "FLAC__stream_encoder_new",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn the_installed_lame_reports_its_version() {
        let lame = LameLibrary::shared().expect("libmp3lame is installed");
        assert!(lame.version().starts_with("3."), "{}", lame.version());
    }

    /// The metadata blocks of a FLAC stream: `(type, body)`.
    fn flac_blocks(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
        assert_eq!(&bytes[..4], b"fLaC");
        let mut pos = 4;
        let mut blocks = Vec::new();
        loop {
            let header = bytes[pos];
            let len = usize::from(bytes[pos + 1]) << 16
                | usize::from(bytes[pos + 2]) << 8
                | usize::from(bytes[pos + 3]);
            blocks.push((header & 0x7F, bytes[pos + 4..pos + 4 + len].to_vec()));
            pos += 4 + len;
            if header & 0x80 != 0 {
                return blocks;
            }
        }
    }

    /// A Vorbis comment body: the vendor and the comments.
    fn vorbis_comments(body: &[u8]) -> (String, Vec<String>) {
        let u32_at = |p: usize| u32::from_le_bytes(body[p..p + 4].try_into().unwrap()) as usize;
        let vendor_len = u32_at(0);
        let vendor = String::from_utf8(body[4..4 + vendor_len].to_vec()).unwrap();
        let mut pos = 4 + vendor_len;
        let count = u32_at(pos);
        pos += 4;
        let mut comments = Vec::new();
        for _ in 0..count {
            let len = u32_at(pos);
            comments.push(String::from_utf8(body[pos + 4..pos + 4 + len].to_vec()).unwrap());
            pos += 4 + len;
        }
        (vendor, comments)
    }

    /// An encoder into memory for `channels` of 16 bits at 48 kHz, at compression level 5.
    fn flac_encoder(channels: u32) -> FlacEncoder<Cursor<Vec<u8>>> {
        let library = FlacLibrary::shared().expect("libFLAC is installed");
        let mut encoder = FlacEncoder::new(library, Cursor::new(Vec::new())).unwrap();
        encoder.set_channels(channels).unwrap();
        encoder.set_sample_rate(48_000).unwrap();
        encoder.set_bits_per_sample(16).unwrap();
        encoder.set_compression_level(5).unwrap();
        encoder
    }

    /// Asserts that `err` is libFLAC's report of `operation` with `message` in it.
    fn assert_flac_call(err: FlacEncodeError, operation: &str, message: &str) {
        match err {
            FlacEncodeError::Codec(CodecError::Call {
                library: "libFLAC",
                operation: op,
                message: m,
            }) if op == operation && m.contains(message) => {}
            other => panic!("expected {operation}: {message}, got {other:?}"),
        }
    }

    #[test]
    fn libflac_writes_streaminfo_comments_and_its_vendor_and_rewrites_streaminfo_at_the_end() {
        let mut encoder = flac_encoder(2);
        encoder
            .set_vorbis_comments(&[("title", "Tíťle".as_bytes()), ("software", b"S\0cut")])
            .unwrap();
        encoder.init_stream().unwrap();
        // A sawtooth and its inverse, 10 000 frames, fed in pieces that end mid-block.
        let samples: Vec<i32> = (0..10_000)
            .flat_map(|i| {
                let v = (i * 97 % 4001 - 2000) * 8;
                [v, -v]
            })
            .collect();
        for piece in samples.chunks(2048) {
            encoder.process_interleaved(piece).unwrap();
        }
        let bytes = encoder.finish().unwrap().into_inner();
        let blocks = flac_blocks(&bytes);
        assert_eq!(blocks.iter().map(|(t, _)| *t).collect::<Vec<_>>(), [0, 4]);
        let info = &blocks[0].1;
        assert_eq!(
            info[..4],
            [0x10, 0x00, 0x10, 0x00],
            "block size 4096 (level 5)"
        );
        let frame_sizes = &info[4..10];
        assert_ne!(frame_sizes, [0; 6], "finish rewrote the frame sizes");
        let packed = u64::from_be_bytes(info[10..18].try_into().unwrap());
        assert_eq!(packed >> 44, 48_000, "rate");
        assert_eq!((packed >> 41) & 7, 1, "two channels");
        assert_eq!((packed >> 36) & 31, 15, "16 bits");
        assert_eq!(packed & ((1 << 36) - 1), 10_000, "total samples");
        assert_ne!(info[18..34], [0; 16], "finish rewrote the MD5");
        let (vendor, comments) = vorbis_comments(&blocks[1].1);
        assert_eq!(
            comments,
            ["title=Tíťle", "software=S"],
            "a value ends at NUL"
        );
        if cfg!(target_os = "macos") {
            assert_eq!(vendor, "reference libFLAC 1.4.3 20230623");
        } else {
            assert!(vendor.starts_with("reference libFLAC "), "{vendor}");
        }
    }

    #[test]
    fn without_comments_libflac_writes_an_empty_comment_block() {
        let mut encoder = flac_encoder(1);
        encoder.init_stream().unwrap();
        encoder.process_interleaved(&[0, 1, -1, 2]).unwrap();
        let bytes = encoder.finish().unwrap().into_inner();
        let blocks = flac_blocks(&bytes);
        assert_eq!(blocks.iter().map(|(t, _)| *t).collect::<Vec<_>>(), [0, 4]);
        let (vendor, comments) = vorbis_comments(&blocks[1].1);
        assert!(vendor.starts_with("reference libFLAC "), "{vendor}");
        assert_eq!(comments, Vec::<String>::new());
    }

    #[test]
    fn an_invalid_configuration_fails_at_init_with_the_status() {
        let mut encoder = flac_encoder(2);
        // FLAC__MAX_SAMPLE_RATE is 2^20 - 1 (0 is a valid "unknown" rate since 1.4).
        encoder.set_sample_rate(1_048_576).unwrap();
        assert_flac_call(
            encoder.init_stream().unwrap_err(),
            "FLAC__stream_encoder_init_stream",
            "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_SAMPLE_RATE",
        );
        let mut encoder = flac_encoder(2);
        encoder.set_bits_per_sample(0).unwrap();
        assert_flac_call(
            encoder.init_stream().unwrap_err(),
            "FLAC__stream_encoder_init_stream",
            "FLAC__STREAM_ENCODER_INIT_STATUS_INVALID_BITS_PER_SAMPLE",
        );
        let library = FlacLibrary::shared().unwrap();
        let mut encoder = FlacEncoder::new(library, Cursor::new(Vec::new())).unwrap();
        assert_flac_call(
            encoder.init_stream().unwrap_err(),
            "FLAC__stream_encoder_init_stream",
            "the channel count is not set",
        );
    }

    #[test]
    fn samples_outside_the_bit_depth_or_partial_frames_are_refused() {
        let mut encoder = flac_encoder(2);
        assert_flac_call(
            encoder.process_interleaved(&[0, 0]).unwrap_err(),
            "FLAC__stream_encoder_process_interleaved",
            "FLAC__STREAM_ENCODER_UNINITIALIZED",
        );
        encoder.init_stream().unwrap();
        assert_flac_call(
            encoder.process_interleaved(&[0, 0, 0]).unwrap_err(),
            "FLAC__stream_encoder_process_interleaved",
            "3 samples are not whole frames of 2",
        );
        assert_flac_call(
            encoder.process_interleaved(&[0, 32_768]).unwrap_err(),
            "FLAC__stream_encoder_process_interleaved",
            "FLAC__STREAM_ENCODER_CLIENT_ERROR",
        );
    }

    #[test]
    fn a_comment_that_is_not_utf8_is_refused() {
        let mut encoder = flac_encoder(2);
        let err = encoder
            .set_vorbis_comments(&[("software", b"\xff\xfe")])
            .unwrap_err();
        assert!(
            matches!(
                &err,
                CodecError::Call {
                    operation: "FLAC__metadata_object_vorbiscomment_entry_from_name_value_pair",
                    message,
                    ..
                } if message.contains("software")
            ),
            "{err:?}"
        );
        assert!(
            encoder.set_vorbis_comments(&[]).is_err(),
            "the block is set once"
        );
    }

    /// An output that fails every write.
    struct FullDisk;

    impl Write for FullDisk {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("disk full"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for FullDisk {
        fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
            Ok(0)
        }
    }

    #[test]
    fn an_output_error_is_reported_as_the_io_error() {
        let library = FlacLibrary::shared().expect("libFLAC is installed");
        let mut encoder = FlacEncoder::new(library, FullDisk).unwrap();
        encoder.set_channels(1).unwrap();
        match encoder.init_stream() {
            Err(FlacEncodeError::Io(err)) => assert_eq!(err.to_string(), "disk full"),
            other => panic!("expected the I/O error, got {other:?}"),
        }
    }
}
