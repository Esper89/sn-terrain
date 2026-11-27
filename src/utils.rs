//! General-purpose tools used by this crate.
//!
//! This module is for code used by this crate that isn't specific to Subnautica terrain modding.

use std::mem::{self, ManuallyDrop};

mod cpu;

pub(crate) mod simd;

pub use cpu::{Cpu, CpuLevel, CpuDispatch, dispatch_cpu, dispatch_cpu_all};

#[expect(unused)]
pub(crate) use cpu::CpuSealed;

#[cfg(test)]
pub(crate) use cpu::test_all_cpu_levels;

// SAFETY: See `mem::transmute`.
pub(crate) const unsafe fn generic_transmute<T, U>(input: T) -> U {
    debug_assert!(mem::size_of::<T>() == mem::size_of::<U>());

    union Transmute<T, U> {
        input: ManuallyDrop<T>,
        output: ManuallyDrop<U>,
    }

    // SAFETY: Required by contract.
    unsafe { ManuallyDrop::into_inner(Transmute { input: ManuallyDrop::new(input) }.output) }
}
