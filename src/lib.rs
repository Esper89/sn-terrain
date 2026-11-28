#![warn(
    missing_docs, clippy::undocumented_unsafe_blocks, clippy::ptr_as_ptr, clippy::manual_assert,
)]
#![allow(
    clippy::missing_safety_doc, clippy::type_complexity,
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
mod rasterize;
mod derasterize;
mod world;

pub mod utils;

#[cfg(feature = "ffi")] pub mod ffi;
#[cfg(feature = "editor")] pub mod editor;

pub use game::{GameDir, Game};
pub use octree::{Octree, SharedOctree, Octnode};
pub use valid::{MaterialIndex, SignedDistance};
pub use voxels::{VoxelGrid, Voxel, ValidVoxelGrid, ValidVoxel};
pub use rasterize::rasterize;
pub use derasterize::{OctreeBuffer, DerasterScratch, derasterize};
pub use world::{World, WorldCache, BatchOctrees, BatchPos, OctreeOffset, OctreePos};

pub mod error {
    //! Error types produced in this crate.

    #[cfg(feature = "ffi")] pub use super::ffi::error::*;
    pub use super::game::error::*;
    pub use super::valid::error::*;
    pub use super::voxels::error::*;
}

#[cfg(test)]
mod test {
    use std::alloc;
    use super::*;
    use crate::utils::test_all_cpu_levels;

    #[test]
    fn raster_deraster_roundtrip() {
        test_all_cpu_levels!(|cpu| {
            let mut voxels = VoxelGrid::EMPTY;
            let mut scratch = DerasterScratch::new(cpu);

            for OctreeCase { name, octree, grid, .. } in OctreeCase::all() {
                rasterize(cpu, octree, &mut voxels);
                let deraster = derasterize::<Box<Octree>>(&mut scratch, &voxels, alloc::Global);
                assert_eq!(deraster.nodes(), octree.nodes(), "{name} ({cpu:?}) raster -> deraster");

                let deraster = derasterize::<Box<Octree>>(&mut scratch, &grid, alloc::Global);
                rasterize(cpu, &deraster, &mut voxels);
                assert_eq!(&voxels, grid, "{name} ({cpu:?}) deraster -> raster");
            }
        });
    }

    pub struct OctreeCase {
        pub name: &'static str,
        pub octree: &'static Octree,
        pub grid: VoxelGrid,
        pub vanilla: Option<&'static Octree>,
    }

    impl OctreeCase {
        pub const fn all() -> &'static [OctreeCase] {
            macro_rules! include_case {
                ($name:literal $(,)?) => {
                    OctreeCase {
                        name: $name,
                        octree: include_octree!(concat!("../tests/cases/octree/", $name, ".bin")),
                        grid: include_voxel_grid!(concat!("../tests/cases/grid/", $name, ".bin")),
                        vanilla: None,
                    }
                };
            }

            macro_rules! include_vanilla_case {
                ($name:literal $(,)?) => {
                    OctreeCase {
                        name: $name,
                        octree: include_octree!(concat!("../tests/cases/octree/", $name, ".bin")),
                        grid: include_voxel_grid!(concat!("../tests/cases/grid/", $name, ".bin")),
                        vanilla: Some({
                            include_octree!(concat!("../tests/cases/vanilla/", $name, ".bin"))
                        }),
                    }
                };
            }

            const {
                &[
                    OctreeCase {
                        name: "solid",
                        octree: Octree::from_root_const::<1, 0>(),
                        grid: VoxelGrid::all(Voxel::new(1, 0)),
                        vanilla: None,
                    },
                    include_case!("sphere"),
                    include_vanilla_case!("big1"),
                    include_vanilla_case!("big2"),
                    include_vanilla_case!("big3"),
                    include_vanilla_case!("big4"),
                    include_vanilla_case!("terrain1"),
                    include_vanilla_case!("terrain2"),
                    include_vanilla_case!("terrain3"),
                    include_vanilla_case!("terrain4"),
                    include_vanilla_case!("terrain5"),
                    include_vanilla_case!("terrain6"),
                    include_vanilla_case!("terrain7"),
                    include_vanilla_case!("terrain8"),
                    OctreeCase {
                        name: "empty",
                        octree: Octree::EMPTY,
                        grid: VoxelGrid::EMPTY,
                        vanilla: None,
                    },
                ]
            }
        }
    }
}
