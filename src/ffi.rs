//! Tools for interacting with other languages, particularly C# and Unity.
//!
//! Anything that would be of particular use to a dynamic library running in a Subnautica process
//! can be found here.

use std::{alloc, ffi, num, ptr};

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
