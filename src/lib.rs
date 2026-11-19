#![warn(missing_docs)]
//! Subnautica modding tools for terrain editing and patching.
//!
//! For information on the binary formats used by this crate, see the [Subnautica terrain data
//! format][data-fmt] specification and the [Subnautica terrain patch format][patch-fmt]
//! specification.
//!
//! [data-fmt]: https://esper89.github.io/Subnautica-TerrainPatcher/docs/terrain-data-format.html
//! [patch-fmt]: https://esper89.github.io/Subnautica-TerrainPatcher/docs/terrain-patch-format.html

#[cfg(not(target_endian = "little"))]
compile_error!("big-endian targets are not supported");

mod octree;
mod valid;
mod voxels;

pub use octree::{Octree, Octnode};
pub use valid::{MaterialIndex, SignedDistance};
pub use voxels::{VoxelGrid, Voxel, ValidVoxelGrid, ValidVoxel};

pub mod error {
    //! Error types produced in this crate.

    pub use super::valid::error::*;
    pub use super::voxels::error::*;
}
