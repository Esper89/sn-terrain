use std::{fmt, fs, io, mem, path::Path};
use crate::{GameDir, Octree, SharedOctree};

/// A terrain directory of batch files.
#[derive(Debug, Clone)]
pub struct World {
    game: GameDir,
}

impl World {
    /// Create a world from a game directory.
    pub fn new(game: GameDir) -> Self {
        World { game }
    }

    /// Load a batch from the game directory by its position.
    ///
    /// Blocks the thread to read from the filesystem.
    pub fn load_batch_blocking(&self, batch: BatchPos) -> BatchOctrees {
        let mut octrees = [SharedOctree::EMPTY; 125];
        self.load_octrees_blocking(batch, |at, octree| {
            octrees[at.idx()] = SharedOctree::new(octree);
        });
        BatchOctrees { octrees }
    }

    /// Load the octrees of a batch from the game directory by its position, receiving each octree.
    ///
    /// Blocks the thread to read from the filesystem.
    pub fn load_octrees_blocking(
        &self,
        batch: BatchPos,
        mut octrees: impl FnMut(OctreeOffset, &Octree),
    ) {
        let BatchPos { x, y, z } = batch;
        let rearrange_north_edge = z == 25;
        let path = self.game.terrain_dir().join(format!("compiled-batch-{x}-{y}-{z}.optoctrees"));
        let bytes = read_batch(&path).unwrap_or(vec![]);

        // FIXME: log errors

        let mut bytes = &bytes[..];
        let mut idx = 0;
        while let Some(at) = OctreeOffset::new(idx) && !bytes.is_empty() {
            let (octree, rem) = Octree::parse(bytes);
            bytes = rem;

            if let Some(octree) = octree {
                octrees(at, octree);
                idx += 1;
            }

            if rearrange_north_edge && (idx + 2).is_multiple_of(5) {
                for _ in 0..2 {
                    let Some(at) = OctreeOffset::new(idx) else { break };
                    octrees(at, Octree::EMPTY);
                    idx += 1;
                }
            }
        }

        while let Some(at) = OctreeOffset::new(idx) {
            octrees(at, Octree::EMPTY);
            idx += 1;
        }
    }
}

fn read_batch(path: &Path) -> io::Result<Vec<u8>> {
    use io::Read;
    let mut file = fs::File::open(path)?;
    let mut buf = [0; 4];
    file.read_exact(&mut buf)?;
    if u32::from_le_bytes(buf) != 4 { return Ok(vec![]) }

    let len = file.metadata().map(|meta| meta.len().saturating_sub(4) as usize).unwrap_or(0);
    let mut buf = Vec::with_capacity(len);
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

/// A batch of terrain octrees.
///
/// Batches are 5 × 5 × 5 grids of octrees. This type is implemented as an array of
/// [`SharedOctree`]s.
///
/// This type is somewhat large, so be careful with moving it by value.
#[derive(Debug, Clone)]
pub struct BatchOctrees {
    octrees: [SharedOctree; 125],
}

impl BatchOctrees {
    /// Access an octree by its offset in the batch.
    pub fn get(&self, at: OctreeOffset) -> &SharedOctree {
        &self.octrees[at.idx()]
    }

    /// Iterate the octrees in this batch by index.
    ///
    /// Octrees are iterated by their [`OctreeOffset`] index ordering.
    pub fn octrees(&self) -> impl Iterator<Item = &SharedOctree> { self.octrees.iter() }
}

/// The X, Y, and Z position of a batch in the world.
///
/// The unit is batches. In-game, these coordinates are relative to the terrain origin, which is at
/// (−2048.0, −3040.0, −2048.0) in Unity engine coordinates.
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
    bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(C)]
pub struct BatchPos {
    /// The X coordinate of a batch in the world.
    pub x: i32,

    /// The Y coordinate of a batch in the world.
    pub y: i32,

    /// The Z coordinate of a batch in the world.
    pub z: i32,
}

impl BatchPos {
    /// The batch at the terrain origin.
    ///
    /// In-game, this is at (−2048.0, −3040.0, −2048.0) in Unity engine coordinates.
    pub const ORIGIN: Self = BatchPos::new(0, 0, 0);

    /// Create a batch position from its coordinates.
    pub const fn new(x: i32, y: i32, z: i32) -> Self { BatchPos { x, y, z } }
}

/// The X, Y, and Z position of an octree in the world.
///
/// The unit is octrees. In-game, these coordinates are relative to the terrain origin, which is at
/// (−2048.0, −3040.0, −2048.0) in Unity engine coordinates.
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
    bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(C)]
pub struct OctreePos {
    /// The X coordinate of an octree in the world.
    pub x: i32,

    /// The Y coordinate of an octree in the world.
    pub y: i32,

    /// The Z coordinate of an octree in the world.
    pub z: i32,
}

impl OctreePos {
    /// The octree at the terrain origin.
    ///
    /// In-game, this is at (−2048.0, −3040.0, −2048.0) in Unity engine coordinates.
    pub const ORIGIN: Self = OctreePos::new(0, 0, 0);

    /// Create an octree position from its coordinates.
    pub const fn new(x: i32, y: i32, z: i32) -> Self { OctreePos { x, y, z } }

    /// Convert this octree position into a batch position and an octree offset.
    pub const fn into_batch_offset(self) -> (BatchPos, OctreeOffset) {
        let OctreePos { x, y, z } = self;
        let [bx, by, bz] = [x.div_euclid(5), y.div_euclid(5), z.div_euclid(5)];
        let [ox, oy, oz] = [x.rem_euclid(5), y.rem_euclid(5), z.rem_euclid(5)];
        let [ox, oy, oz] = [ox as usize, oy as usize, oz as usize];

        let batch = BatchPos::new(bx, by, bz);

        // SAFETY: `ox`, `oy`, and `oz` are all within range 0–4, and `4 * 25 + 4 * 5 + 4` is 124,
        // which is within range 0–124.
        let octree = unsafe { OctreeOffset::new_unchecked(ox * 25 + oy * 5 + oz) };

        (batch, octree)
    }
}

/// An octree's offset within a batch.
///
/// This value is a [`usize`] index guaranteed to be in range 0–124, representing a spatial offset
/// of (0, 0, 0) to (4, 4, 4).
///
/// The octrees in a batch are indexed in Z-Y-X order, like so:
///
/// | `idx` | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | … | 124 |
/// |-------|---|---|---|---|---|---|---|---|---|---|----|----|---|-----|
/// |  `x`  | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0  | 0  | … | 4   |
/// |  `y`  | 0 | 0 | 0 | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 2  | 2  | … | 4   |
/// |  `z`  | 0 | 1 | 2 | 3 | 4 | 0 | 1 | 2 | 3 | 4 | 0  | 1  | … | 4   |
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
    bytemuck::NoUninit, bytemuck::CheckedBitPattern, bytemuck::Zeroable,
)]
#[repr(transparent)]
pub struct OctreeOffset(ValidOctreeOffset);

impl OctreeOffset {
    /// The octree offset with an index of 0 and a spatial offset of (0, 0, 0).
    pub const MIN: Self = OctreeOffset::new_const::<0>();

    /// The octree offset with an index of 124 and a spatial offset of (4, 4, 4).
    pub const MAX: Self = OctreeOffset::new_const::<124>();

    /// Create an octree offset from an index.
    ///
    /// Returns `None` if the index is outside range 0–124.
    pub const fn new(idx: usize) -> Option<Self> {
        if idx < 125 {
            // SAFETY: `idx` is in range 0–124.
            unsafe { Some(OctreeOffset::new_unchecked(idx)) }
        } else {
            None
        }
    }

    /// Create an octree offset from a constant index.
    ///
    /// Panics at compile-time if the constant index is not a valid octree offset in range 0–124.
    pub const fn new_const<const IDX: usize>() -> Self {
        const {
            match OctreeOffset::new(IDX) {
                Some(off) => off,
                _ => panic!("invalid batch octree offset"),
            }
        }
    }

    /// Create an octree offset from an index without checking if it's valid.
    ///
    /// # Safety
    ///
    /// `idx` must be in range 0–124.
    pub const unsafe fn new_unchecked(idx: usize) -> Self {
        // SAFETY: Guaranteed by safety contract.
        let idx = unsafe { mem::transmute::<usize, ValidOctreeOffset>(idx) };
        OctreeOffset(idx)
    }

    /// The index of this octree in the batch.
    ///
    /// Guaranteed to be in range 0–124.
    pub const fn idx(self) -> usize { self.0 as usize }

    /// The positional offset of this octree relative to the start of the batch.
    ///
    /// Guaranteed to be from (0, 0, 0) to (4, 4, 4).
    pub const fn offset(self) -> [i32; 3] {
        const fn calc_offset(idx: usize) -> [i32; 3] {
            let idx = idx as i32;
            [idx / 25, idx % 25 / 5, idx % 5]
        }

        const TABLE: [[i32; 3]; 125] = seq_macro::seq!(N in 0..125 {
            [#(const { calc_offset(N) },)*]
        });

        TABLE[self.idx()]
    }

    /// All octree offsets, 0–125.
    pub const fn all() -> &'static [OctreeOffset; 125] {
        seq_macro::seq!(N in 0..125 {
            const { &[#(OctreeOffset::new_const::<N>(),)*] }
        })
    }
}

impl fmt::Display for OctreeOffset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{}", self.idx())
    }
}

// SAFETY: `OctreeOffset` is a `usize` that must be in range 0–124.
unsafe impl bytemuck::Contiguous for OctreeOffset {
    type Int = usize;
    const MIN_VALUE: usize = 0;
    const MAX_VALUE: usize = 124;
}

seq_macro::seq!(N in 0..125 {
    #[derive(
        Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default,
        bytemuck::NoUninit, bytemuck::CheckedBitPattern, bytemuck::Contiguous, bytemuck::Zeroable,
    )]
    #[repr(usize)]
    enum ValidOctreeOffset { #[default] #(_~N = N,)* }
});

impl fmt::Debug for ValidOctreeOffset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&(*self as usize), f)
    }
}

impl fmt::Display for ValidOctreeOffset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&(*self as usize), f)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Octnode;

    #[test]
    fn load_batch() {
        let world = World::new(GameDir::new("tests/cases/game/sn").expect("should load sn game"));
        let solid = Octree::new_array(&[Octnode { mat: 1, dist: 0, child: 0 }]).nodes();
        let empty = Octree::EMPTY.nodes();

        let batch = world.load_batch_blocking(BatchPos::new(0, 0, 0));
        for idx in OctreeOffset::all() {
            assert_eq!(batch.get(*idx).nodes(), solid);
        }

        let batch = world.load_batch_blocking(BatchPos::new(0, 0, 1));
        assert_eq!(batch.get(OctreeOffset::MIN).nodes(), solid);
        for idx in OctreeOffset::all().iter().skip(1) {
            assert_eq!(batch.get(*idx).nodes(), empty);
        }

        let batch = world.load_batch_blocking(BatchPos::new(0, 0, 25));
        for idx in OctreeOffset::all() {
            let expect = if idx.idx() % 5 > 2 { empty } else { solid };
            assert_eq!(batch.get(*idx).nodes(), expect);
        }
    }
}
