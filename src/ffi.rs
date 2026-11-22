//! Tools for interacting with other languages, particularly C# and Unity.
//!
//! Anything that would be of particular use to a dynamic library running in a Subnautica process
//! can be found here.

use std::{alloc, env, ffi, num, ptr};
use crate::{error::InvalidGameDir, GameDir, Octree, SharedOctree};

/// Get the game directory of the currently running process.
///
/// This is intended to be called while running in Subnautica's address space, for example from a
/// dynamic library loaded by a mod.
pub fn current_process_game_dir() -> Result<GameDir, CurrentProcessInvalidGameDir> {
    let dir = env::current_exe()
        .map(|mut path| { path.pop(); path })
        .map(|path| if path.is_empty() { ".".into() } else { path })
        .unwrap_or(".".into());

    GameDir::new(dir).map_err(CurrentProcessInvalidGameDir::new)
}

/// Extended functionality for shared octrees.
pub trait SharedOctreeExt: Sized + seal_shared_octree_ext::Sealed {
    /// Consume this shared octree, returning a stable pointer to it.
    ///
    /// The pointer must not be written to, but will remain valid for reads until it's passed to
    /// [`from_ptr`](SharedOctreeExt::from_ptr).
    ///
    /// Inline octrees may have their invalid fields modified.
    fn into_ptr(self) -> *const Octree;

    /// Create a shared octree from a pointer produced by [`into_ptr`](SharedOctreeExt::into_ptr).
    ///
    /// The pointer (and pointers derived from it) are no longer valid for reads after calling this
    /// function.
    ///
    /// # Safety
    ///
    /// `ptr` must have been returned from a previous call to
    /// [`into_ptr`](SharedOctreeExt::into_ptr) and must still be valid. The same pointer may not
    /// be passed to this function multiple times.
    unsafe fn from_ptr(ptr: *const Octree) -> Self;
}

mod seal_shared_octree_ext {
    // SAFETY: The type must be `SharedOctree` and the implementation of `SharedOctreeExt` must be
    // correct.
    pub unsafe trait Sealed { }
}
use seal_shared_octree_ext::Sealed as SharedOctreeExtSealed;

// SAFETY: The type is `SharedOctree` and the implementation of `SharedOctreeExt` is correct.
unsafe impl SharedOctreeExtSealed for SharedOctree { }

/// An allocator representing Unity's native persistent allocator.
///
/// Implements [`Allocator`](alloc::Allocator). Note that this allocator **may choose not to shrink
/// allocations**, so make sure that won't cause a memory leak.
#[derive(Debug, Copy, Clone)]
pub struct UnityAlloc(UnityAllocImpl);

#[derive(Debug, Copy, Clone)]
struct UnityAllocImpl {
    // SAFETY: `size` must be non-zero and `align` must be a power of two.
    alloc: unsafe extern "C" fn(size: i64, align: i32) -> *mut ffi::c_void,

    // SAFETY: `ptr` must be a pointer that was previously returned from a call to the corresponding
    // `alloc` function.
    free: unsafe extern "C" fn(ptr: *mut ffi::c_void),
}

impl UnityAlloc {
    /// Create a new Unity allocator from a pair of C function pointers.
    ///
    /// # Safety
    ///
    /// The safety contracts of `alloc` and `free` must be as follows:
    ///
    /// - The `size` argument of `alloc` is non-zero.
    /// - The `align` argument of `alloc` is a power of two.
    /// - The `ptr` argument of `free` is a pointer that was previously returned from a call to the
    ///   corresponding `alloc` function.
    ///
    /// Additionally, the pointer returned by `alloc` must be either null (on allocation failure) or
    /// a valid pointer to a unique allocation, aligned to `align` bytes, that guarantees exclusive
    /// access to read and write for `size` bytes past the pointer, until the pointer is passed back
    /// to `free`.
    ///
    /// The function pointers must be safe to call from any thread, and it must be safe to call them
    /// multiple times concurrently on multiple threads.
    pub unsafe fn new(
        alloc: unsafe extern "C" fn(size: i64, align: i32) -> *mut ffi::c_void,
        free: unsafe extern "C" fn(ptr: *mut ffi::c_void),
    ) -> Self {
        UnityAlloc(UnityAllocImpl { alloc, free })
    }
}

// SAFETY: Implementation is correct as required by this trait.
unsafe impl alloc::Allocator for UnityAlloc {
    fn allocate(&self, layout: alloc::Layout) -> Result<ptr::NonNull<[u8]>, alloc::AllocError> {
        alloc::Allocator::allocate(&self.0, layout)
    }

    unsafe fn deallocate(&self, ptr: ptr::NonNull<u8>, layout: alloc::Layout) {
        // SAFETY: Guaranteed by this trait's safety contract.
        unsafe { alloc::Allocator::deallocate(&self.0, ptr, layout) }
    }

    unsafe fn grow(
        &self,
        ptr: ptr::NonNull<u8>,
        old_layout: alloc::Layout,
        new_layout: alloc::Layout,
    ) -> Result<ptr::NonNull<[u8]>, alloc::AllocError> {
        let skip = old_layout.size() == new_layout.size() && (
            old_layout.align() >= new_layout.align() ||
            ptr.addr().get().is_multiple_of(new_layout.align())
        );

        if skip { Ok(ptr::NonNull::slice_from_raw_parts(ptr, old_layout.size())) }
        else {
            // SAFETY: Guaranteed by this trait's safety contract.
            unsafe { alloc::Allocator::grow(&self.0, ptr, old_layout, new_layout) }
        }
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: ptr::NonNull<u8>,
        old_layout: alloc::Layout,
        new_layout: alloc::Layout,
    ) -> Result<ptr::NonNull<[u8]>, alloc::AllocError> {
        let skip = old_layout.size() == new_layout.size() && (
            old_layout.align() >= new_layout.align() ||
            ptr.addr().get().is_multiple_of(new_layout.align())
        );

        if skip { Ok(ptr::NonNull::slice_from_raw_parts(ptr, old_layout.size())) }
        else {
            // SAFETY: Guaranteed by this trait's safety contract.
            unsafe { alloc::Allocator::grow_zeroed(&self.0, ptr, old_layout, new_layout) }
        }
    }

    unsafe fn shrink(
        &self,
        ptr: ptr::NonNull<u8>,
        old_layout: alloc::Layout,
        new_layout: alloc::Layout,
    ) -> Result<ptr::NonNull<[u8]>, alloc::AllocError> {
        let skip = old_layout.align() >= new_layout.align() ||
            ptr.addr().get().is_multiple_of(new_layout.align());

        if skip { Ok(ptr::NonNull::slice_from_raw_parts(ptr, old_layout.size())) }
        else {
            // SAFETY: Guaranteed by this trait's safety contract.
            unsafe { alloc::Allocator::shrink(&self.0, ptr, old_layout, new_layout) }
        }
    }
}

// SAFETY: Implementation is correct as required by this trait.
unsafe impl alloc::Allocator for UnityAllocImpl {
    fn allocate(&self, layout: alloc::Layout) -> Result<ptr::NonNull<[u8]>, alloc::AllocError> {
        let align = num::NonZero::new(layout.align()).unwrap_or(num::NonZero::<usize>::MIN);

        if layout.size() == 0 {
            let dangling = ptr::NonNull::without_provenance(align);
            Ok(ptr::NonNull::slice_from_raw_parts(dangling, 0))
        } else {
            let size = layout.size().try_into().map_err(|_| alloc::AllocError)?;
            let align = align.get().try_into().map_err(|_| alloc::AllocError)?;

            // SAFETY: `size` is not zero and `align` is a power of two.
            let ptr = unsafe { (self.alloc)(size, align) }.cast::<u8>();
            let ptr = ptr::NonNull::new(ptr).ok_or(alloc::AllocError)?;
            Ok(ptr::NonNull::slice_from_raw_parts(ptr, layout.size()))
        }
    }

    unsafe fn deallocate(&self, ptr: ptr::NonNull<u8>, layout: alloc::Layout) {
        if layout.size() != 0 {
            // SAFETY: `ptr` is currently allocated by this allocator.
            unsafe { (self.free)(ptr.as_ptr().cast::<ffi::c_void>()) }
        }
    }
}

use error::*;
pub(super) mod error {
    use super::*;

    /// The current process is not running from a valid game directory.
    #[derive(Debug, Clone, thiserror::Error)]
    #[error("current process not in a valid game directory: {err}")]
    pub struct CurrentProcessInvalidGameDir {
        err: InvalidGameDir,
    }

    impl CurrentProcessInvalidGameDir {
        pub(super) fn new(err: InvalidGameDir) -> Self {
            CurrentProcessInvalidGameDir { err }
        }

        /// The invalid game directory.
        pub fn err(&self) -> &InvalidGameDir { &self.err }
    }
}
