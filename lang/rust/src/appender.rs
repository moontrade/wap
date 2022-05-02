use core::{mem, ptr};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::null_mut;

use crate::message;
use crate::alloc::{Allocator, Doubled, Global, Grow, TruncResult};
use crate::block::Block;
use crate::header::Header;
use crate::message::{Builder, FlexMessage, Message};

pub struct Appender<
    M: FlexMessage,
    G: Grow = Doubled,
    A: Allocator = Global
> {
    root: *mut u8,
    end: *mut u8,
    trash: usize,
    _p: PhantomData<(M, G, A)>,
}

impl<M, G, A> Appender<M, G, A>
    where
        M: FlexMessage,
        G: Grow,
        A: Allocator {
    pub fn new(extra: usize) -> Option<Self> {
        let (root, end) = M::alloc_with_extra::<A>(extra);
        if root == null_mut() {
            None
        } else {
            Some(Self { root, end, trash: 0, _p: PhantomData })
        }
    }

    pub unsafe fn wrap(root: *mut u8, end: *mut u8) -> Self {
        Self { root, end, trash: 0, _p: PhantomData }
    }
}

impl<M, G, A> Drop for Appender<M, G, A>
    where
        M: FlexMessage,
        G: Grow,
        A: Allocator {
    fn drop(&mut self) {
        A::deallocate(self.root_ptr(), self.capacity());
    }
}

impl<M, G, A> Builder for Appender<M, G, A>
    where
        M: FlexMessage,
        G: Grow,
        A: Allocator {
    type Message = M;
    type Header = M::Header;
    type Block = M::Block;
    type Grow = G;
    type Allocator = A;

    #[inline(always)]
    fn root_ptr(&self) -> *mut u8 {
        self.root
    }

    #[inline(always)]
    fn end_ptr(&self) -> *mut u8 {
        self.end
    }

    #[inline(always)]
    fn take(self) -> (*const u8, *const u8) {
        let root = self.root_ptr();
        let end = self.end;
        mem::forget(self);
        (root, end)
    }

    fn truncate(&mut self, by: usize) -> TruncResult {
        let current_capacity = self.capacity();
        let new_capacity = G::calc(current_capacity, by);

        if new_capacity > Self::Block::SIZE_LIMIT {
            return TruncResult::Overflow;
        }
        let existing_size = self.size();

        // can't shrink
        if new_capacity < current_capacity {
            return TruncResult::Underflow;
        }

        // try to reallocate
        let (new_buffer, new_buffer_capacity) = A::reallocate(self.root_ptr(), new_capacity);
        if new_buffer == ptr::null_mut() {
            return TruncResult::OutOfMemory;
        }

        // in place reallocation?
        if self.root_ptr() == new_buffer {
            // ensure the end is correctly set
            self.end = unsafe { new_buffer.offset(new_capacity as isize) };
            return TruncResult::Success;
        }

        self.root = new_buffer;
        self.end = unsafe { new_buffer.offset(new_capacity as isize) };
        TruncResult::Success
    }

    #[inline(always)]
    fn allocate(&mut self, size: usize) -> *mut Self::Block {
        self.append(size)
    }

    fn append(&mut self, size: usize) -> *mut Self::Block {
        let current_size = self.size();
        let new_size = current_size + Self::Block::OVERHEAD as usize;
        let mut new_tail = unsafe { self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize) };

        if new_tail > self.end {
            match self.truncate(size + Self::Block::OVERHEAD as usize) {
                TruncResult::Success => {
                    new_tail = unsafe { self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize) };
                }
                _ => return ptr::null_mut(),
            }
        }

        let block = Self::Block::from_ptr(self.tail_ptr());

        // update entire message size
        unsafe { M::Header::put_size(self.root_ptr(), new_size); }
        // set the block's size
        block.set_size_usize(size);

        unsafe { block as *mut Self::Block }
    }

    fn reallocate(&mut self, block: &mut Self::Block, size: usize) -> *mut Self::Block {
        if block.is_free() {
            return ptr::null_mut();
        }
        let current_size = self.size();
        let offset = Self::Block::offset(self.root_ptr(), block);

        // out of bounds?
        if offset < 0 || offset > current_size as isize - Self::Block::MIN_SIZE {
            return ptr::null_mut();
        }

        let current_block_size = block.size_usize();

        // Is it the last allocation?
        if current_size == offset as usize + Self::Block::OVERHEAD as usize + current_block_size {
            let new_size = current_size - current_block_size + size;

            if current_block_size >= size {
                unsafe { M::Header::put_size(self.root_ptr(), new_size); }
                return block;
            }

            let mut new_tail = unsafe {
                self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize)
            };
            // Resize required?
            if new_tail > self.end {
                match self.truncate(size + Self::Block::OVERHEAD as usize) {
                    TruncResult::Success => {
                        new_tail = unsafe { self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize) };
                    }
                    _ => return ptr::null_mut(),
                }

                unsafe { M::Header::put_size(self.root_ptr(), new_size); }
                let block = Self::Block::from_ptr(
                    unsafe { self.offset(offset) }
                );
                block.set_size_usize(size);
                return block;
            } else {
                unsafe { M::Header::put_size(self.root_ptr(), new_size); }
                block.set_size_usize(size);
                return block;
            }
        }

        if current_block_size >= size {
            self.trash += current_block_size - size;
            return block;
        }

        let new_block = self.append(size);
        if new_block == ptr::null_mut() {
            return ptr::null_mut();
        }
        // deref block based on original offset since append may have to create a new allocation
        let block = Self::Block::from_ptr(unsafe { self.offset(offset) });
        unsafe {
            ptr::copy_nonoverlapping(
                block.data_ptr(),
                (&*new_block).data_ptr() as *mut u8,
                core::cmp::min(size, current_block_size));
        }

        block.set_free();
        self.trash += current_block_size + Self::Block::OVERHEAD as usize;

        new_block
    }

    fn deallocate(&mut self, block: &mut Self::Block) {
        let current_size = self.size();
        let offset = Self::Block::offset(self.root_ptr(), block);

        // out of bounds?
        if offset < 0 || offset > current_size as isize - Self::Block::MIN_SIZE {
            return;
        }

        if block.is_free() {
            return;
        }

        let block_size = block.size_usize();
        if current_size == offset as usize + Self::Block::OVERHEAD as usize + block_size {
            unsafe { M::Header::put_size(self.root_ptr(), offset as usize); }
            return;
        }

        self.trash += block_size + Self::Block::OVERHEAD as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}