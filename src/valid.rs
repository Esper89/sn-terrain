use std::{fmt, mem};
use crate::Game;

/// A valid one-byte material index value in the game's terrain format.
///
/// This value is guaranteed to have a material index number (and byte value) in the range 1–254.
///
/// `Option<MaterialIndex>::None` is guaranteed to be represented as byte value 0.
///
/// Valid material indices may not map to valid materials when used in-game, depending on which game
/// is running and which mods are installed. This must be handled carefully, as invalid terrain
/// materials can crash the game's world streamer.
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
    bytemuck::NoUninit, bytemuck::CheckedBitPattern,
)]
#[repr(transparent)]
pub struct MaterialIndex(ValidMaterialByte);

impl MaterialIndex {
    /// The default material index that is valid in every game, #1, with a byte value of 1.
    pub const DEFAULT: Self = MaterialIndex::new_const::<1>();

    /// Create a material index from a byte value.
    ///
    /// Returns `Ok(Some)` for a valid material index byte, `Ok(None)` for zero, and `Err`
    /// otherwise.
    pub const fn new(byte: u8) -> Result<Option<Self>, InvalidMaterialByte> {
        if byte == 0 { Ok(None) }
        else if byte == 255 { Err(InvalidMaterialByte::new()) }
        else {
            // SAFETY: `byte` is in range 1–254.
            unsafe { Ok(Some(MaterialIndex::new_unchecked(byte))) }
        }
    }

    /// Create a material index from a constant byte value.
    ///
    /// Panics at compile-time if the constant byte value is not a valid material index.
    pub const fn new_const<const BYTE: u8>() -> Self {
        const {
            match MaterialIndex::new(BYTE) {
                Ok(Some(mat)) => mat,
                _ => panic!("invalid terrain material index"),
            }
        }
    }

    /// Create a material index from a byte value without checking if it's valid.
    ///
    /// # Safety
    ///
    /// `byte` must be in range 1–254.
    pub const unsafe fn new_unchecked(byte: u8) -> Self {
        // SAFETY: `byte` is in range 1–254 per contract. All bit-patterns of `u8` in range 1–254
        // are valid for `ValidMaterialByte`. Both types have the same layout.
        unsafe { MaterialIndex(mem::transmute::<u8, ValidMaterialByte>(byte)) }
    }

    /// Create a material index from a possibly-invalid byte value.
    ///
    /// Invalid byte values are aliased to valid ones. Returns `None` if the byte value is 0,
    /// indicating empty terrain.
    pub const fn from_invalid(byte: u8) -> Option<Self> {
        if byte == 0 { None }
        else {
            let byte = if byte == 255 { 1 } else { byte };

            // SAFETY: `byte` is in range 1–254.
            unsafe { Some(MaterialIndex::new_unchecked(byte)) }
        }
    }

    /// The value of the material index.
    ///
    /// This is also the byte value.
    pub const fn index(self) -> u8 { self.0 as u8 }

    /// If this material index is assigned and valid in an unmodded game.
    ///
    /// In Subnautica, most materials are assigned, and only a few are unassigned:
    ///
    /// - Assigned: 1–94, 97–106, 109–116, 120, 124–133, 140–167, 170–176, 180–254; total **233**.
    /// - Unassigned: 95, 96, 107, 108, 117–119, 121–123, 134–139, 168, 169, 177–179; total **21**.
    ///
    /// Whereas in Below Zero, more materials are unassigned than assigned:
    ///
    /// - Assigned: 1, 85–93, 100–107, 109–114, 121–139, 150–155, 159–180, 182–185, 200–202,
    ///   205–214, 217–241, 250; total **114**.
    /// - Unassigned: 2–84, 94–99, 108, 115–120, 140–149, 156–158, 181, 186–199, 203, 204, 215, 216,
    ///   242–249, 251–254; total **140**.
    ///
    /// Note that a material being valid or invalid in-game depends not only on which game is
    /// running, but also on the set of installed mods. Invalid terrain materials must be handled
    /// carefully, as they can crash the game's world streamer.
    pub const fn is_vanilla(self, game: Game) -> bool {
        match game {
            Game::Subnautica => match self.index() {
                1..=94 | 97..=106 | 109..=116 | 120 | 124..=133 | 140..=167 | 170..=176 | 180..=254
                    => true,

                95 | 96 | 107 | 108 | 117..=119 | 121..=123 | 134..=139 | 168 | 169 | 177..=179
                    => false,

                0 | 255 => unreachable!(),
            },

            Game::BelowZero => match self.index() {
                1 | 85..=93 | 100..=107 | 109..=114 | 121..=139 | 150..=155 | 159..=180 |
                182..=185 | 200..=202 | 205..=214 | 217..=241 | 250
                    => true,

                2..=84 | 94..=99 | 108 | 115..=120 | 140..=149 | 156..=158 | 181 | 186..=199 | 203 |
                204 | 215 | 216 | 242..=249 | 251..=254
                    => false,

                0 | 255 => unreachable!(),
            },
        }
    }

    /// All material indices, 1–254.
    pub const fn all() -> &'static [Self; 254] {
        seq_macro::seq!(N in 1..=254 {
            const { &[#(MaterialIndex::new_const::<N>(),)*] }
        })
    }

    /// All material indices that are assigned and valid in an unmodded game.
    ///
    /// Subnautica has a total of **233** assigned materials: 1–94, 97–106, 109–116, 120, 124–133,
    /// 140–167, 170–176, 180–254.
    ///
    /// Below Zero has a total of **114** assigned materials: 1, 85–93, 100–107, 109–114, 121–139,
    /// 150–155, 159–180, 182–185, 200–202, 205–214, 217–241, 250.
    ///
    /// Note that a material being valid or invalid in-game depends not only on which game is
    /// running, but also on the set of installed mods. Invalid terrain materials must be handled
    /// carefully, as they can crash the game's world streamer.
    pub const fn all_vanilla(game: Game) -> &'static [Self] {
        macro_rules! mats {
            ($($n:literal),* $(,)?) => {
                const { &[$(MaterialIndex::new_const::<$n>()),*] }
            };
        }

        match game {
            Game::Subnautica => mats![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44,
                45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
                66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
                87, 88, 89, 90, 91, 92, 93, 94, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 109,
                110, 111, 112, 113, 114, 115, 116, 120, 124, 125, 126, 127, 128, 129, 130, 131, 132,
                133, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155,
                156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 170, 171, 172, 173, 174,
                175, 176, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194,
                195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211,
                212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228,
                229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245,
                246, 247, 248, 249, 250, 251, 252, 253, 254,
            ],

            Game::BelowZero => mats![
                1, 85, 86, 87, 88, 89, 90, 91, 92, 93, 100, 101, 102, 103, 104, 105, 106, 107, 109,
                110, 111, 112, 113, 114, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132,
                133, 134, 135, 136, 137, 138, 139, 150, 151, 152, 153, 154, 155, 159, 160, 161, 162,
                163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179,
                180, 182, 183, 184, 185, 200, 201, 202, 205, 206, 207, 208, 209, 210, 211, 212, 213,
                214, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232,
                233, 234, 235, 236, 237, 238, 239, 240, 241, 250,
            ],
        }
    }

    /// All material indices that are unassigned and invalid in an unmodded game.
    ///
    /// Subnautica has a total of **21** unassigned materials: 95, 96, 107, 108, 117–119, 121–123,
    /// 134–139, 168, 169, 177–179.
    ///
    /// Below Zero has a total of **140** unassigned materials: 2–84, 94–99, 108, 115–120, 140–149,
    /// 156–158, 181, 186–199, 203, 204, 215, 216, 242–249, 251–254.
    ///
    /// Note that a material being valid or invalid in-game depends not only on which game is
    /// running, but also on the set of installed mods. Invalid terrain materials must be handled
    /// carefully, as they can crash the game's world streamer.
    pub const fn all_vanilla_unassigned(game: Game) -> &'static [Self] {
        macro_rules! mats {
            ($($n:literal),* $(,)?) => {
                const { &[$(MaterialIndex::new_const::<$n>()),*] }
            };
        }

        match game {
            Game::Subnautica => mats![
                95, 96, 107, 108, 117, 118, 119, 121, 122, 123, 134, 135, 136, 137, 138, 139, 168,
                169, 177, 178, 179,
            ],

            Game::BelowZero => mats![
                2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
                25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
                46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66,
                67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 94, 95, 96,
                97, 98, 99, 108, 115, 116, 117, 118, 119, 120, 140, 141, 142, 143, 144, 145, 146,
                147, 148, 149, 156, 157, 158, 181, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195,
                196, 197, 198, 199, 203, 204, 215, 216, 242, 243, 244, 245, 246, 247, 248, 249, 251,
                252, 253, 254,
            ],
        }
    }
}

impl fmt::Display for MaterialIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{}", self.index())
    }
}

// SAFETY: `MaterialIndex` is a `u8` that must be in range 1–254.
unsafe impl bytemuck::Contiguous for MaterialIndex {
    type Int = u8;
    const MIN_VALUE: u8 = 1;
    const MAX_VALUE: u8 = 254;
}

// SAFETY: `Option<MaterialIndex>::None` is `0` per constant assertion.
unsafe impl bytemuck::ZeroableInOption for MaterialIndex { }

const _: () = {
    if mem::size_of::<Option<MaterialIndex>>() != 1 {
        panic!("option material niche optimization not active")
    }

    // SAFETY: `Option<MaterialIndex>` is one byte in size and that byte is always initialized.
    let niche = unsafe { mem::transmute::<Option<MaterialIndex>, u8>(None) };
    if niche != 0 { panic!("option material niche optimization picked wrong niche") }
};

seq_macro::seq!(N in 1..=254 {
    #[derive(
        Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
        bytemuck::NoUninit, bytemuck::CheckedBitPattern, bytemuck::Contiguous,
    )]
    #[repr(u8)]
    enum ValidMaterialByte { #[default] #(_~N = N,)* }
});

impl fmt::Debug for ValidMaterialByte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&(*self as u8), f)
    }
}

impl fmt::Display for ValidMaterialByte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&(*self as u8), f)
    }
}

/// A valid one-byte signed distance value in the game's terrain format.
///
/// This value is guaranteed to have a byte value in the range 1–252, representing a signed distance
/// in the range (−1.0, 1.0].
///
/// `Option<SignedDistance>::None` is guaranteed to be represented as byte value 0.
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash,
    bytemuck::NoUninit, bytemuck::CheckedBitPattern,
)]
#[repr(transparent)]
pub struct SignedDistance(ValidDistanceByte);

impl SignedDistance {
    /// The minimum signed distance value, representing −125/126 ≈ −0.99206, with a byte value of 1.
    pub const MIN: Self = SignedDistance::new_const::<1>();

    /// The halfway point signed distance value, representing 0.0, with a byte value of 126.
    pub const HALF: Self = SignedDistance::new_const::<126>();

    /// The maximum signed distance value, representing +1.0, with a byte value of 252.
    pub const MAX: Self = SignedDistance::new_const::<252>();

    /// Create a signed distance from a byte value.
    ///
    /// Returns `Ok(Some)` for a valid signed distance byte, `Ok(None)` for zero, and `Err`
    /// otherwise.
    pub const fn new(byte: u8) -> Result<Option<Self>, InvalidDistanceByte> {
        if byte == 0 { Ok(None) }
        else if byte > 252 { Err(InvalidDistanceByte::new(byte)) }
        else {
            // SAFETY: `byte` is in range 1–252.
            unsafe { Ok(Some(SignedDistance::new_unchecked(byte))) }
        }
    }

    /// Create a signed distance from a constant byte value.
    ///
    /// Panics at compile-time if the constant byte value is not a valid signed distance.
    pub const fn new_const<const BYTE: u8>() -> Self {
        const {
            match SignedDistance::new(BYTE) {
                Ok(Some(mat)) => mat,
                _ => panic!("invalid terrain signed distance"),
            }
        }
    }

    /// Create a signed distance from a byte value without checking if it's valid.
    ///
    /// # Safety
    ///
    /// `byte` must be in range 1–252.
    pub const unsafe fn new_unchecked(byte: u8) -> Self {
        // SAFETY: `byte` is in range 1–252 per contract. All bit-patterns of `u8` in range 1–252
        // are valid for `ValidDistanceByte`. Both types have the same layout.
        unsafe { SignedDistance(mem::transmute::<u8, ValidDistanceByte>(byte)) }
    }

    /// Create a signed distance from a possibly-invalid byte value.
    ///
    /// Invalid byte values are clamped to valid ones. Returns `None` if the byte value is 0,
    /// indicating a solid point.
    pub const fn from_invalid(byte: u8) -> Option<Self> {
        if byte == 0 { None }
        else {
            let byte = if byte > 252 { 252 } else { byte };

            // SAFETY: `byte` is in range 1–252.
            unsafe { Some(SignedDistance::new_unchecked(byte)) }
        }
    }

    /// Create a signed distance from a numeric value, as a signed integer fraction over 126.
    ///
    /// Returns `None` if the value is outside the range −125–+126.
    pub const fn from_i8_over_126(dist: i8) -> Option<Self> {
        let byte = dist as isize + 126;
        if byte < 1 || byte > 252 { None } else {
            match SignedDistance::new(byte as u8) {
                Ok(dist) => dist,
                Err(_) => None,
            }
        }
    }

    /// Create a signed distance from a numeric value, as a 32-bit floating point number.
    ///
    /// Returns `None` if the value is outside the range (−1.0, +1.0] or is not a number.
    pub const fn from_f32(dist: f32) -> Option<Self> {
        if dist <= 1.0 && dist > -1.0 {
            let byte = (dist * 126.0 + 126.0).round_ties_even() as u8;
            let byte = if byte < 1 { 1 } else if byte > 252 { 252 } else { byte };

            // SAFETY: `byte` is in range 1–252.
            Some(unsafe { SignedDistance::new_unchecked(byte) })
        } else {
            None
        }
    }

    /// Create a signed distance from a numeric value, as a 64-bit floating point number.
    ///
    /// Returns `None` if the value is outside the range (−1.0, +1.0] or is not a number.
    pub const fn from_f64(dist: f64) -> Option<Self> {
        if dist <= 1.0 && dist > -1.0 {
            let byte = (dist * 126.0 + 126.0).round_ties_even() as u8;
            let byte = if byte < 1 { 1 } else if byte > 252 { 252 } else { byte };

            // SAFETY: `byte` is in range 1–252.
            Some(unsafe { SignedDistance::new_unchecked(byte) })
        } else {
            None
        }
    }

    /// Create a signed distance from a numeric value, as a fraction over 126.
    ///
    /// If the value is outside the range −125–+126 it is clamped within range.
    pub const fn clamp_i8_over_126(dist: i8) -> Self {
        let mut byte = dist as isize + 126;
        if byte > 252 { byte = 252; }
        else if byte < 1 { byte = 1; }
        let byte = byte as u8;

        // SAFETY: `byte` is within range 1–252.
        unsafe { SignedDistance::new_unchecked(byte) }
    }

    /// Create a signed distance from a numeric value, as a 32-bit floating point number.
    ///
    /// If the value is outside the range (−1.0, +1.0] it is clamped within range. If the value is
    /// not a number, [`HALF`](SignedDistance::HALF) is returned.
    pub const fn clamp_f32(dist: f32) -> Self {
        let byte = (dist * 126.0 + 126.0).round_ties_even().clamp(1.0, 252.0) as u8;
        match SignedDistance::new(byte) {
            Ok(Some(dist)) => dist,
            _ => SignedDistance::HALF,
        }
    }

    /// Create a signed distance from a numeric value, as a 64-bit floating point number.
    ///
    /// If the value is outside the range (−1.0, +1.0] it is clamped within range. If the value is
    /// not a number, [`HALF`](SignedDistance::HALF) is returned.
    pub const fn clamp_f64(dist: f64) -> Self {
        let byte = (dist * 126.0 + 126.0).round_ties_even().clamp(1.0, 252.0) as u8;
        match SignedDistance::new(byte) {
            Ok(Some(dist)) => dist,
            _ => SignedDistance::HALF,
        }
    }

    /// The byte value of this signed distance.
    pub const fn byte(self) -> u8 { self.0 as u8 }

    /// The numeric value of this signed distance, as a signed integer fraction over 126.
    ///
    /// This value is guaranteed to be in the range −125–+126.
    pub const fn to_i8_over_126(self) -> i8 { (self.byte() as isize - 126) as i8 }

    /// The numeric value of this signed distance as a 32-bit floating point number.
    ///
    /// This value is guaranteed to be in the range (−1.0, +1.0].
    pub const fn to_f32(self) -> f32 { (self.byte() as f32 - 126.0) / 126.0 }

    /// The numeric value of this signed distance as a 64-bit floating point number.
    ///
    /// This value is guaranteed to be in the range (−1.0, +1.0].
    pub const fn to_f64(self) -> f64 { (self.byte() as f64 - 126.0) / 126.0 }

    /// If this signed distance value is negative.
    pub const fn is_negative(self) -> bool { self.byte() < 126 }

    /// If this signed distance value is positive.
    pub const fn is_positive(self) -> bool { self.byte() > 126 }

    /// If this signed distance value is exactly [`HALF`](SignedDistance::HALF).
    pub const fn is_half(self) -> bool { self.byte() == 126 }

    /// If this signed distance value indicates an interior point.
    ///
    /// [`HALF`](SignedDistance::HALF) is considered interior.
    ///
    /// Guaranteed to be the opposite of [`is_exterior`](SignedDistance::is_exterior).
    pub const fn is_interior(self) -> bool { !self.is_exterior() }

    /// If this signed distance value indicates an exterior point.
    ///
    /// [`HALF`](SignedDistance::HALF) is not considered exterior.
    ///
    /// Guaranteed to be the opposite of [`is_interior`](SignedDistance::is_interior).
    pub const fn is_exterior(self) -> bool { self.is_negative() }

    /// The minimum of two signed distance values.
    pub const fn min(self, other: Self) -> Self {
        if self.byte() > other.byte() { other } else { self }
    }

    /// The maximum of two signed distance values.
    pub const fn max(self, other: Self) -> Self {
        if self.byte() < other.byte() { other } else { self }
    }
}

/// The default signed distance is [`HALF`](SignedDistance::HALF).
impl Default for SignedDistance {
    fn default() -> Self { SignedDistance::HALF }
}

// SAFETY: `SignedDistance` is a `u8` that must be in range 1–252.
unsafe impl bytemuck::Contiguous for SignedDistance {
    type Int = u8;
    const MIN_VALUE: u8 = 1;
    const MAX_VALUE: u8 = 252;
}

// SAFETY: `Option<SignedDistance>::None` is `0` per constant assertion.
unsafe impl bytemuck::ZeroableInOption for SignedDistance { }

const _: () = {
    if mem::size_of::<Option<SignedDistance>>() != 1 {
        panic!("option distance niche optimization not active")
    }

    // SAFETY: `Option<SignedDistance>` is one byte in size and that byte is always initialized.
    let niche = unsafe { mem::transmute::<Option<SignedDistance>, u8>(None) };
    if niche != 0 { panic!("option distance niche optimization picked wrong niche") }
};

seq_macro::seq!(N in 1..=252 {
    #[derive(
        Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
        bytemuck::NoUninit, bytemuck::CheckedBitPattern, bytemuck::Contiguous,
    )]
    #[repr(u8)]
    enum ValidDistanceByte { #[default] #(_~N = N,)* }
});

impl fmt::Debug for ValidDistanceByte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&(*self as u8), f)
    }
}

impl fmt::Display for ValidDistanceByte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&(*self as u8), f)
    }
}

use error::*;
pub(super) mod error {
    use super::*;

    /// An invalid byte was found where a valid material index was expected.
    #[derive(Clone, thiserror::Error)]
    #[error("invalid terrain material index byte: 255")]
    pub struct InvalidMaterialByte(());

    impl fmt::Debug for InvalidMaterialByte {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_tuple("InvalidMaterialByte").field(&self.byte()).finish()
        }
    }

    impl InvalidMaterialByte {
        pub(super) const fn new() -> Self { InvalidMaterialByte(()) }

        /// The invalid byte, 255.
        pub const fn byte(&self) -> u8 { 255 }
    }

    /// An invalid byte was found where a valid signed distance was expected.
    #[derive(Debug, Clone, thiserror::Error)]
    #[error("invalid terrain signed distance byte: {}", *.0 as u8)]
    pub struct InvalidDistanceByte(InvalidDistanceByteImpl);

    #[derive(Copy, Clone)]
    #[repr(u8)]
    enum InvalidDistanceByteImpl { _253 = 253, _254 = 254, _255 = 255 }

    impl fmt::Debug for InvalidDistanceByteImpl {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Debug::fmt(&(*self as u8), f)
        }
    }

    impl InvalidDistanceByte {
        #[inline(always)]
        pub(super) const fn new(b: u8) -> Self {
            InvalidDistanceByte(match b {
                253 => InvalidDistanceByteImpl::_253,
                254 => InvalidDistanceByteImpl::_254,
                255 => InvalidDistanceByteImpl::_255,
                _ => unreachable!(),
            })
        }

        /// The invalid byte, in range 253–255.
        pub const fn byte(&self) -> u8 { self.0 as u8 }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn material_index_all_vanilla() {
        let (mut sn_a, mut sn_u, mut bz_a, mut bz_u) = (
            MaterialIndex::all_vanilla(Game::Subnautica),
            MaterialIndex::all_vanilla_unassigned(Game::Subnautica),
            MaterialIndex::all_vanilla(Game::BelowZero),
            MaterialIndex::all_vanilla_unassigned(Game::BelowZero),
        );

        for mat in MaterialIndex::all() {
            let sn = if mat.is_vanilla(Game::Subnautica) { &mut sn_a } else { &mut sn_u };
            let bz = if mat.is_vanilla(Game::BelowZero) { &mut bz_a } else { &mut bz_u };

            for slice in [sn, bz] {
                let Some((first, rest)) = slice.split_first() else { panic!("too short") };
                assert_eq!(mat, first);
                *slice = rest;
            }
        }

        assert_eq!(sn_a, &[]);
        assert_eq!(sn_u, &[]);
        assert_eq!(bz_a, &[]);
        assert_eq!(bz_u, &[]);
    }
}
