use std::mem;
use crate::{MaterialIndex, SignedDistance};

/// An octree-sized terrain voxel grid with voxels in the game's data format.
///
/// The grid is 32 × 32 × 32 and is indexed along a Z-order curve. The contained voxels may be
/// invalid; for a voxel grid that's always valid, see [`ValidVoxelGrid`].
///
/// This type is both [`Copy`] and quite large. Be careful with moving it by value.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C, align(64))]
pub struct VoxelGrid(pub [Voxel; 32 * 32 * 32]);

impl VoxelGrid {
    /// A zeroed grid of empty voxels.
    pub const EMPTY: Self = VoxelGrid([Voxel::EMPTY; _]);

    /// Calculate the X, Y, and Z coordinates of a voxel from its index in the grid.
    ///
    /// Output coordinates will be in range 0–31. Input index must be in range 1–32 767.
    /// Out-of-range inputs may cause panics or nonsense results.
    pub const fn coords(idx: usize) -> [i32; 3] { voxel_grid_coords(idx) }

    /// Calculate the index of a voxel in the grid from its X, Y, and Z coordinates.
    ///
    /// Output index will be in range 1–32 767. Input coordinates should be in range 0–31.
    /// Out-of-range inputs may cause panics or nonsense results.
    pub const fn index([x, y, z]: [i32; 3]) -> usize { voxel_grid_index([x, y, z]) }
}

impl Default for VoxelGrid {
    fn default() -> Self { VoxelGrid::EMPTY }
}

/// A two-byte terrain voxel in the game's data format.
///
/// The fields of this type may be invalid; for a voxel that's always valid, see [`ValidVoxel`].
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C, align(2))]
pub struct Voxel {
    /// Material index of this voxel, or zero for an empty voxel.
    ///
    /// Byte value 255 is invalid.
    ///
    /// Valid material indices may not map to valid materials when used in-game, depending on which
    /// game is running and which mods are installed. This must be handled carefully, as invalid
    /// terrain materials can crash the game's world streamer.
    pub mat: u8,

    /// Signed distance from this voxel to the isosurface in range 1–252, encoded as
    /// `d * 126 + 126`, or zero for a voxel that's far from the isosurface.
    ///
    /// Byte values 253–255 are invalid.
    ///
    /// If the material index of this voxel is zero, the signed distance should be zero too.
    pub dist: u8,
}

impl Voxel {
    /// An empty, zeroed voxel.
    pub const EMPTY: Self = Voxel { mat: 0, dist: 0 };
}

/// An octree-sized terrain voxel grid with valid voxels in the game's data format.
///
/// The grid is 32 × 32 × 32 and is indexed along a Z-order curve. The contained voxels are
/// guaranteed to be valid; for voxels that may be invalid, see [`VoxelGrid`].
///
/// This type is both [`Copy`] and quite large. Be careful with moving it by value.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, bytemuck::Zeroable)]
#[repr(C, align(64))]
pub struct ValidVoxelGrid(pub [ValidVoxel; 32 * 32 * 32]);

impl ValidVoxelGrid {
    /// A zeroed grid of empty voxels.
    pub const EMPTY: Self = ValidVoxelGrid([ValidVoxel::empty(); _]);

    /// Calculate the X, Y, and Z coordinates of a voxel from its index in the grid.
    ///
    /// Output coordinates will be in range 0–31. Input index must be in range 1–32 767.
    /// Out-of-range inputs may cause panics or nonsense results.
    pub const fn coords(idx: usize) -> [i32; 3] { voxel_grid_coords(idx) }

    /// Calculate the index of a voxel in the grid from its X, Y, and Z coordinates.
    ///
    /// Output index will be in range 1–32 767. Input coordinates should be in range 0–31.
    /// Out-of-range inputs may cause panics or nonsense results.
    pub const fn index([x, y, z]: [i32; 3]) -> usize { voxel_grid_index([x, y, z]) }
}

impl Default for ValidVoxelGrid {
    fn default() -> Self { ValidVoxelGrid::EMPTY }
}

// SAFETY: `ValidVoxelGrid` contains no uninitialized or padding bytes.
unsafe impl bytemuck::NoUninit for ValidVoxelGrid { }

// SAFETY: All values that pass this check are valid bit-patterns for `ValidVoxelGrid`.
unsafe impl bytemuck::CheckedBitPattern for ValidVoxelGrid {
    type Bits = [u16; 32 * 32 * 32];
    fn is_valid_bit_pattern(bits: &[u16; 32 * 32 * 32]) -> bool {
        bits.iter().all(|v| ValidVoxel::is_valid_bit_pattern(v))
    }
}

/// A valid two-byte terrain voxel in the game's data format.
///
/// The first byte of this value is guaranteed to either be a material index in range 1–254, or to
/// have a byte value of 0 for a fully empty voxel.
///
/// The second byte of this value is guaranteed to either have a byte value in the range 1–252
/// representing a signed distance in the range (−1.0, 1.0], or to have a byte value of 0 for a
/// fully solid or fully empty voxel.
///
/// For a voxel that may have invalid fields, see [`Voxel`].
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default, bytemuck::Zeroable)]
#[repr(C, align(2))]
pub struct ValidVoxel {
    // INVARIANT: If `mat` is `None`, `dist` must also be `None`.
    mat: Option<MaterialIndex>,
    dist: Option<SignedDistance>,
}

impl ValidVoxel {
    /// Create a fully empty voxel.
    ///
    /// Empty voxels are guaranteed to be stored as zeroed bytes.
    pub const fn empty() -> Self {
        ValidVoxel { mat: None, dist: None }
    }

    /// Create a fully solid voxel with a material type.
    pub const fn solid(mat: MaterialIndex) -> Self {
        ValidVoxel { mat: Some(mat), dist: None }
    }

    /// Create a surface voxel with a material type and a signed distance.
    pub const fn surface(mat: MaterialIndex, dist: SignedDistance) -> Self {
        ValidVoxel { mat: Some(mat), dist: Some(dist) }
    }

    /// Create a voxel from byte values.
    ///
    /// Returns `Ok` for a valid voxel and `Err` otherwise.
    pub const fn new(bytes: u16) -> Result<Self, InvalidVoxelBytes> {
        let [mat, dist] = bytes.to_le_bytes();
        let (mat, dist) = (MaterialIndex::new(mat), SignedDistance::new(dist));
        let (Ok(mat), Ok(dist)) = (mat, dist) else { return Err(InvalidVoxelBytes::new(bytes)) };

        if mat.is_none() && dist.is_some() { return Err(InvalidVoxelBytes::new(bytes)) };
        Ok(ValidVoxel { mat, dist })
    }

    /// Create a voxel from constant byte values.
    ///
    /// Panics at compile-time if the constant byte values are not a valid voxel.
    pub const fn new_const<const BYTES: u16>() -> Self {
        const {
            match ValidVoxel::new(BYTES) {
                Ok(vox) => vox,
                _ => panic!("invalid terrain voxel"),
            }
        }
    }

    /// Create a voxel from byte values without checking if they're valid.
    ///
    /// # Safety
    ///
    /// The least significant byte must be either 0 or a valid [`MaterialIndex`] in range 1–254, and
    /// the most significant byte must be either 0 or a valid [`SignedDistance`] in range 1–252. If
    /// the least significant byte is 0, the most significant byte must also be 0.
    pub const unsafe fn new_unchecked(bytes: u16) -> Self {
        // SAFETY: Guaranteed by this function's safety contract.
        unsafe { mem::transmute::<u16, ValidVoxel>(bytes) }
    }

    /// Create a voxel from possibly-invalid byte values.
    ///
    /// Invalid byte values are aliased and clamped to valid ones.
    pub const fn from_invalid(bytes: u16) -> Self {
        let [mat, dist] = bytes.to_le_bytes();
        let (mat, dist) = (MaterialIndex::from_invalid(mat), SignedDistance::from_invalid(dist));
        let dist = if mat.is_none() { None } else { dist };
        ValidVoxel { mat, dist }
    }

    /// The material index of this voxel.
    ///
    /// Returns `None` if the voxel is fully empty. If this is `None`, [`dist`](ValidVoxel::dist) is
    /// guaranteed to also be `None`.
    pub const fn mat(self) -> Option<MaterialIndex> { self.mat }

    /// The signed distance of this voxel.
    ///
    /// Returns `None` if the voxel is fully empty or fully solid.
    pub const fn dist(self) -> Option<SignedDistance> { self.dist }

    /// The signed distance of this voxel, clamped to be within range.
    ///
    /// Fully solid voxels get a signed distance of [`MAX`](SignedDistance::MAX) and fully empty
    /// voxels get a signed distance of [`MIN`](SignedDistance::MIN).
    pub const fn clamp_dist(self) -> SignedDistance {
        match self.dist {
            Some(dist) => dist,
            None => if self.mat.is_some() { SignedDistance::MAX } else { SignedDistance::MIN },
        }
    }

    /// If this voxel indicates an interior point.
    ///
    /// Guaranteed to be the opposite of [`is_exterior`](ValidVoxel::is_exterior).
    pub const fn is_interior(self) -> bool {
        match self.dist {
            Some(dist) => dist.is_interior(),
            None => self.mat.is_some(),
        }
    }

    /// If this voxel indicates an exterior point.
    ///
    /// Guaranteed to be the opposite of [`is_interior`](ValidVoxel::is_interior).
    pub const fn is_exterior(self) -> bool { !self.is_interior() }

    /// If this voxel is completely empty of terrain and far from the surface.
    ///
    /// Guaranteed to be the opposite of [`is_nonempty`](ValidVoxel::is_nonempty).
    pub const fn is_empty(self) -> bool { self.mat.is_none() }

    /// If this voxel contains any amount of terrain.
    ///
    /// Guaranteed to be the opposite of [`is_empty`](ValidVoxel::is_empty).
    pub const fn is_nonempty(self) -> bool { !self.is_empty() }

    /// If this voxel is completely full of terrain and far from the surface.
    ///
    /// Guaranteed to be the opposite of [`is_nonsolid`](ValidVoxel::is_nonsolid).
    pub const fn is_solid(self) -> bool { self.mat.is_some() && self.dist.is_none() }

    /// If this voxel is not completely full of terrain.
    ///
    /// Guaranteed to be the opposite of [`is_solid`](ValidVoxel::is_solid).
    pub const fn is_nonsolid(self) -> bool { !self.is_solid() }

    /// If this voxel is near the surface of the terrain.
    ///
    /// Guaranteed to be the opposite of [`is_nonsurface`](ValidVoxel::is_nonsurface).
    pub const fn is_surface(self) -> bool { self.dist.is_some() }

    /// If this voxel is either completely empty of terrain or completely full of terrain, and is
    /// far from the surface.
    ///
    /// Guaranteed to be the opposite of [`is_surface`](ValidVoxel::is_surface).
    pub const fn is_nonsurface(self) -> bool { !self.is_surface() }
}

// SAFETY: There are no uninitialized bytes and no padding bytes in the layout of `ValidVoxel`.
unsafe impl bytemuck::NoUninit for ValidVoxel { }

/// SAFETY: All values that pass this check are valid bit-patterns for `ValidVoxel`.
unsafe impl bytemuck::CheckedBitPattern for ValidVoxel {
    type Bits = u16;
    fn is_valid_bit_pattern(bits: &u16) -> bool { ValidVoxel::new(*bits).is_ok() }
}

const _: () = {
    if mem::size_of::<ValidVoxel>() != 2 || mem::align_of::<ValidVoxel>() != 2 {
        panic!("invalid voxel size or alignment")
    }

    // SAFETY: `ValidVoxel` is two bytes in size and both bytes are always initialized.
    let niches = unsafe { mem::transmute::<ValidVoxel, u16>(ValidVoxel::empty()) };
    if niches != 0 { panic!("voxel option niche optimizations picked wrong niches") }
};

const fn voxel_grid_index([x, y, z]: [i32; 3]) -> usize {
    let [x, y, z] = [x as usize, y as usize, z as usize];

    // FIXME: use `deposit_bits` when it's stabilized
    /*
    z.deposit_bits(0b001_001_001_001_001) |
    y.deposit_bits(0b010_010_010_010_010) |
    x.deposit_bits(0b100_100_100_100_100)
    */

    (z & 0b00001) << 0 |
    (z & 0b00010) << 2 |
    (z & 0b00100) << 4 |
    (z & 0b01000) << 6 |
    (z & 0b10000) << 8 |

    (y & 0b00001) << 1 |
    (y & 0b00010) << 3 |
    (y & 0b00100) << 5 |
    (y & 0b01000) << 7 |
    (y & 0b10000) << 9 |

    (x & 0b00001) << 2 |
    (x & 0b00010) << 4 |
    (x & 0b00100) << 6 |
    (x & 0b01000) << 8 |
    (x & 0b10000) << 10
}

const fn voxel_grid_coords(idx: usize) -> [i32; 3] {
    // FIXME: use `extract_bits` when it's stabilized
    /*
    [
        idx.extract_bits(0b100_100_100_100_100) as i32,
        idx.extract_bits(0b010_010_010_010_010) as i32,
        idx.extract_bits(0b001_001_001_001_001) as i32,
    ]
    */

    let idx = idx as i32;
    [
        (idx >> 2 & 0b00001) |
        (idx >> 4 & 0b00010) |
        (idx >> 6 & 0b00100) |
        (idx >> 8 & 0b01000) |
        (idx >> 10 & 0b10000),

        (idx >> 1 & 0b00001) |
        (idx >> 3 & 0b00010) |
        (idx >> 5 & 0b00100) |
        (idx >> 7 & 0b01000) |
        (idx >> 9 & 0b10000),

        (idx >> 0 & 0b00001) |
        (idx >> 2 & 0b00010) |
        (idx >> 4 & 0b00100) |
        (idx >> 6 & 0b01000) |
        (idx >> 8 & 0b10000),
    ]
}

use error::*;
pub(super) mod error {
    /// Invalid bytes were found where a valid terrain voxel was expected.
    #[derive(Debug, Clone, thiserror::Error)]
    #[error("invalid terrain voxel bytes: {:?}", .0.to_le_bytes())]
    pub struct InvalidVoxelBytes(u16);

    impl InvalidVoxelBytes {
        pub(super) const fn new(bytes: u16) -> Self { InvalidVoxelBytes(bytes) }

        /// The invalid bytes.
        pub const fn bytes(&self) -> u16 { self.0 }
    }
}
