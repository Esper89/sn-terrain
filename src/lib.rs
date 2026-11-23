#![warn(
    missing_docs, clippy::missing_safety_doc,
    clippy::ptr_as_ptr, clippy::undocumented_unsafe_blocks,
)]
//! Subnautica modding tools for terrain editing and patching.
//!
//! For information on the binary formats used by this crate, see the [Subnautica terrain data
//! format][data-fmt] specification and the [Subnautica terrain patch format][patch-fmt]
//! specification.
//!
//! # Flags
//!
//! This crate has the following feature flags:
//!
#![cfg_attr(feature = "ffi", doc = "- `ffi` enables the [`ffi`] module.")]
#![cfg_attr(not(feature = "ffi"), doc = "- `ffi` enables the `ffi` module.")]
#![cfg_attr(feature = "editor", doc = "- `editor` enables the [`editor`] module.")]
#![cfg_attr(not(feature = "editor"), doc = "- `editor` enables the `editor` module.")]
//!
//! [data-fmt]: https://esper89.github.io/Subnautica-TerrainPatcher/docs/terrain-data-format.html
//! [patch-fmt]: https://esper89.github.io/Subnautica-TerrainPatcher/docs/terrain-patch-format.html

#[cfg(not(target_endian = "little"))]
compile_error!("big-endian targets are not supported");

mod game;
mod octree;
mod valid;
mod voxels;
mod world;

#[cfg(feature = "ffi")] pub mod ffi;
#[cfg(feature = "editor")] pub mod editor;

pub use game::{GameDir, Game};
pub use octree::{Octree, SharedOctree, Octnode};
pub use valid::{MaterialIndex, SignedDistance};
pub use voxels::{VoxelGrid, Voxel, ValidVoxelGrid, ValidVoxel};
pub use world::{World, BatchOctrees, BatchPosition, OctreeOffset, OctreePosition};

pub mod error {
    //! Error types produced in this crate.

    #[cfg(feature = "ffi")] pub use super::ffi::error::*;
    pub use super::game::error::*;
    pub use super::valid::error::*;
    pub use super::voxels::error::*;
}
