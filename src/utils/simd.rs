use std::mem;
use fearless_simd::{Fallback, kernel};
use crate::utils;

#[cfg(target_arch = "x86")] use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")] use core::arch::x86_64 as arch;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use fearless_simd::{Sse2, Sse4_2, Avx2, Avx512};

#[cfg(target_arch = "aarch64")]
use fearless_simd::Neon;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use fearless_simd::WasmSimd128;

pub use fearless_simd::prelude::*;
pub use fearless_simd_macros::simd;

#[expect(unused)]
pub use fearless_simd::{
    u8x16, u8x32, u8x64, u16x8, u16x16, u16x32, u32x4, u32x8, u32x16, u64x2, u64x4, u64x8,
    i8x16, i8x32, i8x64, i16x8, i16x16, i16x32, i32x4, i32x8, i32x16, i64x2, i64x4, i64x8,
    mask8x16, mask8x32, mask8x64, mask16x8, mask16x16, mask16x32,
    mask32x4, mask32x8, mask32x16, mask64x2, mask64x4, mask64x8,
    f32x4, f32x8, f32x16, f64x2, f64x4, f64x8,
};

#[inline(always)]
pub const fn cast_ref_as_simd_arr<S: Simd, T, U>(simd: S, input: &T) -> &U
where T: bytemuck::NoUninit, U: CastArray<Item: SimdBase<S>> {
    const {
        assert!(mem::size_of::<T>() == mem::size_of::<U>());
        assert!(mem::align_of::<T>() >= mem::align_of::<U>());
    }
    _ = simd;

    // SAFETY: `T` and `U` have the same size and compatible alignments. `bytemuck::NoUninit`
    // guarantees that `T` is valid to cast from, while `CastArray` and `SimdBase` guarantee that
    // `U` is valid to cast to. We consume `simd` to ensure that we aren't forging an invalid
    // instance of `U`.
    unsafe { &*(&raw const *input).cast() }
}

#[inline(always)]
pub const fn cast_simd_arr<S: Simd, T, U>(input: T) -> U
where T: CastArray<Item: SimdBase<S>>, U: CastArray<Item: SimdBase<S>> {
    const {
        assert!(mem::size_of::<T>() == mem::size_of::<U>());
    }

    // SAFETY: `T` and `U` have the same size. `CastArray` and `SimdBase` guarantee that they both
    // have castable layouts. We are not forging an invalid SIMD token because the input array and
    // output array both use the same SIMD token type.
    unsafe { utils::generic_transmute(input) }
}

#[inline(always)]
pub const fn cast_simd_arr_ref<S: Simd, T, U>(input: &T) -> &U
where T: CastArray<Item: SimdBase<S>>, U: CastArray<Item: SimdBase<S>> {
    const {
        assert!(mem::size_of::<T>() == mem::size_of::<U>());
        assert!(mem::align_of::<T>() >= mem::align_of::<U>());
    }

    // SAFETY: `T` and `U` have the same size and compatible alignments. `CastArray` and `SimdBase`
    // guarantee that they both have castable layouts. We are not forging an invalid SIMD token
    // because the input array and output array both use the same SIMD token type.
    unsafe { &*(&raw const *input).cast() }
}

#[inline(always)]
pub const fn chunk_arr_ref<I, T, U>(input: &T) -> &U
where T: CastArray<Item = I>, U: CastArray<Item: CastArray<Item = I>> {
    const {
        assert!(mem::size_of::<T>() == mem::size_of::<U>());
        assert!(mem::size_of::<I>() != 0);
    }

    // SAFETY: `T` and `U` are arrays of the same type `I`, they have identical sizes, and `I` is
    // not zero-sized, so they must have the same total number of items. Any two types of nested
    // arrays with the same number and type of contained items will have identical, compatible
    // layouts.
    unsafe { &*(&raw const *input).cast() }
}

// SAFETY: The type must be represented in memory as an array of its `Item` type and nothing else.
pub unsafe trait CastArray: Sized {
    type Item;
}

// SAFETY: `[T; _]` is represented in memory as an array of `T`.
unsafe impl<T, const N: usize> CastArray for [T; N] {
    type Item = T;
}

// SAFETY: `gnat::array::Arr<T, _>` is represented in memory as an array of `T`.
unsafe impl<T, N: gnat::Nat> CastArray for gnat::array::Arr<T, N> {
    type Item = T;
}

// SAFETY: `gnat::array::CopyArr<T, _>` is represented in memory as an array of `T`.
unsafe impl<T: Copy, N: gnat::Nat> CastArray for gnat::array::CopyArr<T, N> {
    type Item = T;
}

#[cfg(test)]
pub fn simd_arr_eq<S: Simd, T: SimdBase<S>, const N: usize>(a: &[T; N], b: &[T; N]) -> bool {
    (0..N).map(|i| a[i].simd_eq(b[i]).all_true()).all(|b| b)
}

pub trait SimdExt: Simd {
    type Width: gnat::Nat;

    fn downcast_u8s_arr<const N: usize>(
        self,
        arr: [Self::u8s; N],
        f_x16: impl FnOnce(Self, [u8x16<Self>; N]) -> [u8x16<Self>; N],
        f_x32: impl FnOnce(Self, [u8x32<Self>; N]) -> [u8x32<Self>; N],
        f_x64: impl FnOnce(Self, [u8x64<Self>; N]) -> [u8x64<Self>; N],
    ) -> [Self::u8s; N];

    fn zip_low_within_blocks_u8s(self, a: Self::u8s, b: Self::u8s) -> Self::u8s;
    fn zip_high_within_blocks_u8s(self, a: Self::u8s, b: Self::u8s) -> Self::u8s;
}

impl_simd_ext!(for Fallback {
    type Width = 16;
});

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl_simd_ext!(for Sse2 {
    type Width = 16;
});

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl_simd_ext!(for Sse4_2 {
    type Width = 16;
});

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl_simd_ext!(for Avx2 {
    type Width = 32;

    fn downcast_u8s_arr<const N: usize>(
        token,
        arr: [u8s; N],
        _f_x16: impl FnOnce(Self, [u8x16; N]) -> [u8x16; N],
        f_x32: impl FnOnce(Self, [u8x32; N]) -> [u8x32; N],
        _f_x64: impl FnOnce(Self, [u8x64; N]) -> [u8x64; N],
    ) -> [u8s; N] {
        f_x32(token, arr)
    }

    fn zip_low_within_blocks_u8s(token, a: u8s, b: u8s) -> u8s {
        arch::_mm256_unpacklo_epi8(a.into(), b.into()).simd_into(token)
    }

    fn zip_high_within_blocks_u8s(token, a: u8s, b: u8s) -> u8s {
        arch::_mm256_unpackhi_epi8(a.into(), b.into()).simd_into(token)
    }
});

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl_simd_ext!(for Avx512 {
    type Width = 64;

    fn downcast_u8s_arr<const N: usize>(
        token,
        arr: [u8s; N],
        _f_x16: impl FnOnce(Self, [u8x16; N]) -> [u8x16; N],
        _f_x32: impl FnOnce(Self, [u8x32; N]) -> [u8x32; N],
        f_x64: impl FnOnce(Self, [u8x64; N]) -> [u8x64; N],
    ) -> [u8s; N] {
        f_x64(token, arr)
    }

    fn zip_low_within_blocks_u8s(token, a: u8s, b: u8s) -> u8s {
        arch::_mm512_unpacklo_epi8(a.into(), b.into()).simd_into(token)
    }

    fn zip_high_within_blocks_u8s(token, a: u8s, b: u8s) -> u8s {
        arch::_mm512_unpackhi_epi8(a.into(), b.into()).simd_into(token)
    }
});

#[cfg(target_arch = "aarch64")]
impl_simd_ext!(for Neon {
    type Width = 16;
});

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
impl_simd_ext!(for WasmSimd128 {
    type Width = 16;
});

const _: () = {
    const fn assert_width<S: SimdExt>() {
        assert!(S::u8s::LEN == gnat::to_usize::<S::Width>().unwrap());
    }

    assert_width::<Fallback>();

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        assert_width::<Sse2>();
        assert_width::<Sse4_2>();
        assert_width::<Avx2>();
        assert_width::<Avx512>();
    }

    #[cfg(target_arch = "aarch64")] {
        assert_width::<Neon>();
    }

    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))] {
        assert_width::<WasmSimd128>();
    }
};

macro_rules! impl_simd_ext {
    (for $ty:ident {
        type Width = $width:literal;

        $(fn downcast_u8s_arr<const N: usize>(
            $dc_u8sarr_token:ident,
            $dc_u8sarr_arr:ident: [u8s; N],
            $dc_u8sarr_fx16:ident: impl FnOnce(Self, [u8x16; N]) -> [u8x16; N],
            $dc_u8sarr_fx32:ident: impl FnOnce(Self, [u8x32; N]) -> [u8x32; N],
            $dc_u8sarr_fx64:ident: impl FnOnce(Self, [u8x64; N]) -> [u8x64; N],
        ) -> [u8s; N] {
            $($dc_u8sarr_body:tt)*
        })?

        $(fn zip_low_within_blocks_u8s(
            $zlwb_u8s_token:ident, $zlwb_u8s_a:ident: u8s, $zlwb_u8s_b:ident: u8s $(,)?
        ) -> u8s {
            $($zlwb_u8s_body:tt)*
        })?

        $(fn zip_high_within_blocks_u8s(
            $zhwb_u8s_token:ident, $zhwb_u8s_a:ident: u8s, $zhwb_u8s_b:ident: u8s $(,)?
        ) -> u8s {
            $($zhwb_u8s_body:tt)*
        })?
    }) => {
        impl SimdExt for $ty {
            type Width = gnat::lit!($width);

            impl_simd_ext!(@impl dc_u8sarr for $ty $({
                fn downcast_u8s_arr<const N: usize>(
                    $dc_u8sarr_token,
                    $dc_u8sarr_arr: [<$ty as Simd>::u8s; N],
                    $dc_u8sarr_fx16: impl FnOnce($ty, [u8x16<$ty>; N]) -> [u8x16<$ty>; N],
                    $dc_u8sarr_fx32: impl FnOnce($ty, [u8x32<$ty>; N]) -> [u8x32<$ty>; N],
                    $dc_u8sarr_fx64: impl FnOnce($ty, [u8x64<$ty>; N]) -> [u8x64<$ty>; N],
                ) -> [<$ty as Simd>::u8s; N] {
                    $($dc_u8sarr_body)*
                }
            })?);

            impl_simd_ext!(@impl zlwb_u8s for $ty $({
                fn zip_low_within_blocks_u8s(
                    $zlwb_u8s_token,
                    $zlwb_u8s_a: <$ty as Simd>::u8s, $zlwb_u8s_b: <$ty as Simd>::u8s,
                ) -> <$ty as Simd>::u8s {
                    $($zlwb_u8s_body)*
                }
            })?);

            impl_simd_ext!(@impl zhwb_u8s for $ty $({
                fn zip_high_within_blocks_u8s(
                    $zhwb_u8s_token,
                    $zhwb_u8s_a: <$ty as Simd>::u8s, $zhwb_u8s_b: <$ty as Simd>::u8s,
                ) -> <$ty as Simd>::u8s {
                    $($zhwb_u8s_body)*
                }
            })?);
        }
    };

    (@impl dc_u8sarr for $ty:ident) => {
        impl_simd_ext!(@impl dc_u8sarr for $ty {
            fn downcast_u8s_arr<const N: usize>(
                token,
                arr: [<$ty as Simd>::u8s; N],
                f_x16: impl FnOnce($ty, [u8x16<$ty>; N]) -> [u8x16<$ty>; N],
                _f_x32: impl FnOnce($ty, [u8x32<$ty>; N]) -> [u8x32<$ty>; N],
                _f_x64: impl FnOnce($ty, [u8x64<$ty>; N]) -> [u8x64<$ty>; N],
            ) -> [<$ty as Simd>::u8s; N] {
                f_x16(token, arr)
            }
        });
    };

    (@impl zlwb_u8s for $ty:ident) => {
        impl_simd_ext!(@impl zlwb_u8s for $ty {
            fn zip_low_within_blocks_u8s(
                token, a: <$ty as Simd>::u8s, b: <$ty as Simd>::u8s,
            ) -> <$ty as Simd>::u8s {
                token.zip_low_u8x16(a, b)
            }
        });
    };

    (@impl zhwb_u8s for $ty:ident) => {
        impl_simd_ext!(@impl zhwb_u8s for $ty {
            fn zip_high_within_blocks_u8s(
                token, a: <$ty as Simd>::u8s, b: <$ty as Simd>::u8s,
            ) -> <$ty as Simd>::u8s {
                token.zip_high_u8x16(a, b)
            }
        });
    };

    (@impl $_:ident for Fallback {
        fn $method:ident $(<
            $($(const $const_param:ident: $const_ty:ty),+ $(,)?)?
        >)? ($token:ident $(, $arg:ident: $arg_ty:ty)* $(,)?) $(-> $ret:ty)? {
            $($body:tt)*
        }
    }) => {
        #[inline(always)]
        fn $method<$($($(const $const_param: $const_ty),+)?)?>(
            self $(, $arg: $arg_ty)*,
        ) $(-> $ret)? {
            let $token = self;
            { $($body)* }
        }
    };

    (@impl $_:ident for $ty:ident {
        fn $method:ident $(<
            $($(const $const_param:ident: $const_ty:ty),+ $(,)?)?
        >)? ($token:ident $(, $arg:ident: $arg_ty:ty)* $(,)?) $(-> $ret:ty)? {
            $($body:tt)*
        }
    }) => {
        #[inline(always)]
        fn $method<$($($(const $const_param: $const_ty),+)?)?>(
            self $(, $arg: $arg_ty)*,
        ) $(-> $ret)? {
            kernel! {
                #[inline(always)]
                fn __method_kernel<$($($(const $const_param: $const_ty),+)?)?>(
                    $token: $ty $(, $arg: $arg_ty)*,
                ) $(-> $ret)? {
                    $($body)*
                }
            }

            __method_kernel(self $(, $arg)*)
        }
    };
}
use impl_simd_ext;
