//! General-purpose tools used by this crate.
//!
//! This module is for code used by this crate that isn't specific to Subnautica terrain modding.

mod cpu;

pub use cpu::{Cpu, CpuLevel, CpuDispatch, dispatch_cpu, dispatch_cpu_all};

#[cfg(test)]
pub(crate) use cpu::test_all_cpu_levels;
