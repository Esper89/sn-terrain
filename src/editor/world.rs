use std::{hash, mem, sync::Arc};
use crate::{GameDir, SharedOctree, World, BatchOctrees, BatchPos, OctreePos};

/// A terrain directory of batch files that caches octree loading.
#[derive(Debug)]
pub struct WorldCache {
    world: World,
    cache: BatchCache,
}

#[derive(Debug, Clone)]
struct CachedBatch {
    bytes: usize,
    octrees: Option<Arc<BatchOctrees>>,
}

impl WorldCache {
    /// Create a world cache for the terrain in the specified game directory.
    ///
    /// The maximum capacity is 256 MiB.
    pub fn new(game: GameDir) -> Self {
        WorldCache {
            world: World::new(game),
            cache: new_batch_cache(),
        }
    }

    /// Get an octree by its position, loading it from the game directory if it isn't cached.
    ///
    /// May block the thread to read from the filesystem.
    pub fn get_octree_blocking(&self, octree: OctreePos) -> SharedOctree {
        let (batch, octree) = octree.into_batch_offset();
        let batch = self.cache
            .get_or_insert_with(&batch, || Ok(new_cached_batch(&self.world, batch)))
            .unwrap_or_else(|e| e);

        if let Some(octrees) = batch.octrees {
            octrees.get(octree).clone()
        } else {
            SharedOctree::EMPTY
        }
    }
}

impl Clone for WorldCache {
    fn clone(&self) -> Self {
        WorldCache {
            world: self.world.clone(),
            cache: new_batch_cache(),
        }
    }
}

fn new_cached_batch(world: &World, batch: BatchPos) -> CachedBatch {
    let octrees = world.load_batch_blocking(batch);
    let octrees = if octrees.octrees().all(|o| o.is_empty()) { None } else {
        Some(Arc::new(octrees))
    };

    let mut bytes = const { mem::size_of::<BatchPos>() + mem::size_of::<CachedBatch>() };
    if let Some(octrees) = &octrees {
        bytes += const { mem::size_of::<usize>() * 2 + mem::size_of::<BatchOctrees>() };
        for octree in octrees.octrees() {
            bytes += octree.heap_space();
        }
    }

    CachedBatch { bytes, octrees }
}

type BatchCache = quick_cache::sync::Cache<
    BatchPos, CachedBatch,
    BatchWeighter, hash::BuildHasherDefault<ahash::AHasher>,
>;

fn new_batch_cache() -> BatchCache {
    let options = quick_cache::OptionsBuilder::new()
        .estimated_items_capacity(1 << 12)
        .weight_capacity(1 << 28)
        .hot_allocation(0.9)
        .build()
        .expect("cache options should build");

    quick_cache::sync::Cache::with_options(
        options,
        BatchWeighter,
        hash::BuildHasherDefault::<ahash::AHasher>::new(),
        quick_cache::sync::DefaultLifecycle::default(),
    )
}

#[derive(Debug, Copy, Clone)]
struct BatchWeighter;

impl quick_cache::Weighter<BatchPos, CachedBatch> for BatchWeighter {
    fn weight(&self, _: &BatchPos, v: &CachedBatch) -> u64 { v.bytes as u64 }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Octree, Octnode};

    #[test]
    fn get_octree() {
        let game = GameDir::new("tests/cases/game/sn").expect("should load sn game");
        let world = WorldCache::new(game);
        let solid = Octree::new_array(&[Octnode { mat: 1, dist: 0, child: 0 }]).nodes();
        let empty = Octree::EMPTY.nodes();

        let cases = [
            ([0, 0, 0], solid),
            ([-100, 100, -100], empty),
            ([1, 1, 1], solid),
            ([4, 0, 0], solid),
            ([5, 0, 0], empty),
            ([0, 0, 5], solid),
            ([0, 0, 6], empty),
            ([0, 0, 127], solid),
            ([0, 0, 128], empty),
        ];

        for ([x, y, z], expect) in cases {
            let octree = world.get_octree_blocking(OctreePos::new(x, y, z));
            assert_eq!(octree.nodes(), expect, "octree [{x}, {y}, {z}]");
        }
    }
}
