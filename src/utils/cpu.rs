use std::fmt::Debug;
use fearless_simd::Level as SimdLevel;
use crate::utils::simd::SimdExt;

/// A zero-sized token type indicating the instruction set level available on the current CPU.
///
/// No public types implement this trait, and it is sealed. To get an instance of a type
/// implementing this trait, you will have to implement [`CpuDispatch`] and then call
/// [`dispatch_cpu`] with a [`CpuLevel`].
pub trait Cpu:
    Copy + Debug + Send + Sync + 'static + seal_cpu::Sealed { }

pub(crate) use seal_cpu::Sealed as CpuSealed;
mod seal_cpu {
    use super::*;

    pub trait Sealed {
        type Simd: SimdExt;
        fn simd(self) -> Self::Simd;
    }
}

#[derive(Debug, Copy, Clone)]
struct CpuImpl<S: SimdExt>(S);

impl<S: SimdExt> Cpu for CpuImpl<S> { }
impl<S: SimdExt> CpuSealed for CpuImpl<S> {
    type Simd = S;

    #[inline(always)]
    fn simd(self) -> S { self.0 }
}

/// A value indicating the instruction set level available on the current CPU.
///
/// This should ideally be created once and passed around.
#[derive(Debug, Copy, Clone)]
pub struct CpuLevel {
    simd: SimdLevel,
}

impl CpuLevel {
    /// Detect the best instruction set level available on the current CPU.
    ///
    /// This function caches its output, but you should prefer to store and pass around the output
    /// instead of calling it multiple times.
    pub fn detect() -> Self {
        CpuLevel { simd: SimdLevel::new() }
    }

    /// Get the best statically-supported instruction set level.
    ///
    /// Using this will very likely not give optimal performance on the current CPU.
    pub const fn baseline() -> Self {
        CpuLevel { simd: SimdLevel::baseline() }
    }
}

/// A function that is generic over instruction set levels.
///
/// To call this function, use [`dispatch_cpu`].
pub trait CpuDispatch {
    /// The output type returned by the function.
    type Output;

    /// The generic function that receives a [`Cpu`].
    fn dispatch(self, cpu: impl Cpu) -> Self::Output;
}

/// Dispatch a function that is generic over instruction set levels.
///
/// The provided function will be called at the specified instruction set level. In most cases,
/// using this function will require implementing [`CpuDispatch`].
pub fn dispatch_cpu<R>(level: CpuLevel, dispatch: impl CpuDispatch<Output = R>) -> R {
    use SimdLevel::*;

    match level.simd {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Sse2(simd) => dispatch.dispatch(CpuImpl(simd)),

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Sse4_2(simd) => dispatch.dispatch(CpuImpl(simd)),

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Avx2(simd) => dispatch.dispatch(CpuImpl(simd)),

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Avx512(simd) => dispatch.dispatch(CpuImpl(simd)),

        #[cfg(target_arch = "aarch64")]
        Neon(simd) => dispatch.dispatch(CpuImpl(simd)),

        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        WasmSimd128(simd) => dispatch.dispatch(CpuImpl(simd)),

        _ => dispatch.dispatch(CpuImpl(fearless_simd::Fallback::new())),
    }
}

/// Dispatch a function that is generic over instruction set levels, on every supported instruction
/// set level of the current CPU.
///
/// This is useful for tests.
pub fn dispatch_cpu_all(dispatch: impl CpuDispatch<Output = ()> + Clone) {
    let level = CpuLevel::detect();

    cfg_select! {
        any(target_arch = "x86", target_arch = "x86_64") => {
            if let Some(simd) = level.simd.as_avx512() {
                dispatch.clone().dispatch(CpuImpl(simd));
            }
            if let Some(simd) = level.simd.as_avx2() {
                dispatch.clone().dispatch(CpuImpl(simd));
            }
            if let Some(simd) = level.simd.as_sse4_2() {
                dispatch.clone().dispatch(CpuImpl(simd));
            }
            if let Some(simd) = level.simd.as_sse2() {
                dispatch.clone().dispatch(CpuImpl(simd));
            }
        },

        target_arch = "aarch64" => {
            if let Some(simd) = level.simd.as_neon() {
                dispatch.clone().dispatch(CpuImpl(simd));
            }
        },

        all(target_arch = "wasm32", target_feature = "simd128") => {
            if let Some(simd) = level.simd.as_wasm_simd128() {
                dispatch.clone().dispatch(CpuImpl(simd));
            }
        },

        _ => { },
    }

    dispatch.dispatch(CpuImpl(fearless_simd::Fallback::new()));
}

#[cfg(test)]
macro_rules! test_all_cpu_levels {
    (|$cpu:ident| $expr:expr) => {{
        struct __TestAllCpuLevelsImpl;

        impl ::core::clone::Clone for __TestAllCpuLevelsImpl {
            fn clone(&self) -> Self { __TestAllCpuLevelsImpl }
        }

        impl $crate::utils::CpuDispatch for __TestAllCpuLevelsImpl {
            type Output = ();
            fn dispatch(self, $cpu: impl $crate::utils::Cpu) { $expr }
        }

        $crate::utils::dispatch_cpu_all(__TestAllCpuLevelsImpl);
    }};
}

#[cfg(test)]
pub(crate) use test_all_cpu_levels;
