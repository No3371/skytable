/*
 * Created on Wed Jun 02 2021
 *
 * This file is a part of Skytable
 * Skytable (formerly known as TerrabaseDB or Skybase) is a free and open-source
 * NoSQL database written by Sayan Nandan ("the Author") with the
 * vision to provide flexibility in data modelling without compromising
 * on performance, queryability or scalability.
 *
 * Copyright (c) 2021, Sayan Nandan <ohsayan@outlook.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
*/

#![allow(dead_code)] // TODO(@ohsayan): Remove this lint once we're done

cfg_if::cfg_if! {
    if #[cfg(all(
        target_feature = "sse2",
        any(target_arch = "x86", target_arch = "x86_64")
    ))] {
        use self::sse2 as imp;
    } else {
        use self::generic as imp;
    }
}

#[cfg(all(
    target_feature = "sse2",
    any(target_arch = "x86", target_arch = "x86_64")
))]
mod sse2 {
    //! SSE2 Vectorized implementations of group lookups for hosts that support them
    use super::bitmask::Bitmask;
    use super::control_bytes;

    #[cfg(target_arch = "x86")]
    use core::arch::x86;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64 as x86;
    use core::mem;

    pub type BitmaskWord = u16;
    pub const BITMASK_STRIDE: usize = 1;
    pub const BITMASK_MASK: BitmaskWord = 0xffff;

    #[derive(Clone, Copy)]
    pub struct Group(x86::__m128i);

    pub const WIDTH: usize = mem::size_of::<Group>();

    impl Group {
        /// This will return the size of Self, which is a 128-bit wide integer vector (128-bit SIMD register)
        /// (intel platforms only)
        pub const WIDTH: usize = mem::size_of::<Self>();
        /// Returns a full group
        pub const fn empty_static() -> &'static [u8; Group::WIDTH] {
            #[repr(C)]
            struct AlignedBytes {
                // some explicit padding for alignment to ensure alignment to the group size
                _align: [Group; 0],
                bytes: [u8; Group::WIDTH],
            }
            const ALIGNED_BYTES: AlignedBytes = AlignedBytes {
                _align: [],
                bytes: [control_bytes::EMPTY; Group::WIDTH],
            };
            &ALIGNED_BYTES.bytes
        }

        /// Load a group of bytes starting at the given address (unaligned)
        pub unsafe fn load_unaligned(ptr: *const u8) -> Self {
            Group(x86::_mm_loadu_si128(ptr.cast()))
        }

        /// Load a group of bytes starting at the given address. This is an aligned read,
        /// and guranteed to be aligned to the alignment of the [`Group`]
        pub unsafe fn load_aligned(ptr: *const u8) -> Self {
            Group(x86::_mm_load_si128(ptr.cast()))
        }

        /// Store this group of bytes (self's) at the given address. This must be aligned
        /// to the alignment of the [`Group`]
        pub unsafe fn store_aligned(self, ptr: *mut u8) {
            x86::_mm_store_si128(ptr.cast(), self.0)
        }

        /// Returns a bitmask that gives us which bytes in the group have the
        /// given byte
        pub fn match_byte(self, byte: u8) -> Bitmask {
            unsafe {
                /*
                 _mm_cmpeq_epi8 will two compare 8-bit packed integers in the Group: a __m128i in this case
                 with _mm_set1_epi8 of the byte (basically setting all bits to the provided byte). To be more
                 specific, we're doing this:  `pcmpeqb xmm, xmm` on the two SIMD registers. This result
                 is stored in `cmp`. _mm_movemask_epi8 will return the most significant bit of each 8-bit
                 element in `cmp` (one one the highest end). We then cast this to an unsigned 16-bit integer
                 as _mm_movemask_epi8 returns an i32, with the high order bits zeroed. Finally, we return this
                 as the Bitmask.
                */
                let cmp = x86::_mm_cmpeq_epi8(self.0, x86::_mm_set1_epi8(byte as i8));
                Bitmask(x86::_mm_movemask_epi8(cmp) as u16)
            }
        }

        /// Returns a bitmask with all the bytes in the group which are empty
        pub fn match_empty(self) -> Bitmask {
            self.match_byte(control_bytes::EMPTY)
        }

        /// Returns a bitmask indicating which all bytes in the group were empty or deleted
        pub fn match_empty_or_deleted(self) -> Bitmask {
            unsafe {
                // _mm_movemask_epi8 will again give us the most significant bits
                Bitmask(x86::_mm_movemask_epi8(self.0) as u16)
            }
        }

        /// Returns a bitmask indicating which all bytes in the group are full
        pub fn match_full(&self) -> Bitmask {
            self.match_empty_or_deleted().invert()
        }

        pub fn transform_full_to_deleted_and_special_to_empty(self) -> Self {
            /*
             for high bit = 1 (EMPTY/DELETED) => 1111_1111
             for high bit = 0 (FULL) => 1000_0000

             So we first compute if the byte is special. If high order bit is 1 => special,
             else it isn't special
             Now just apply a bitwise OR on every byte:
             So, 1111_1111 | 1000_0000 => 1111_1111
             And, 0000_0000 | 1000_0000 => 1000_0000
            */

            unsafe {
                // give us our zeroed vector
                let zero = x86::_mm_setzero_si128();
                // compute greater than for the given vectors
                let special = x86::_mm_cmpgt_epi8(zero, self.0);
                // do a lovely bitwise or on each byte (logical explanation is above)
                Group(x86::_mm_or_si128(
                    special,
                    x86::_mm_set1_epi8(0b10000000_u8 as i8),
                ))
            }
        }
    }
}

mod bitmask {
    use super::imp::{BitmaskWord, BITMASK_MASK, BITMASK_STRIDE};
    #[derive(Clone, Copy)]
    pub struct Bitmask(pub BitmaskWord);

    impl Bitmask {
        /// Returns a bitmask with all the bits inverted
        ///
        /// For example (please, _just an example_), if your input is 11 base 10, or
        /// 1011 base 2 -- then your output is 0100 base 2 or 4 base 10. So it's basically
        /// a bitwise NOT on each bit in the integer
        pub fn invert(self) -> Self {
            Self(self.0 ^ BITMASK_MASK)
        }

        /// Flips the bits of the Bitmask at the given index
        pub unsafe fn flip(&mut self, index: usize) -> bool {
            let mask = 1 << (index * BITMASK_STRIDE + BITMASK_STRIDE - 1);
            self.0 ^= mask;
            self.0 & mask == 0
        }

        /// Returns the lowest bit set in he bitmask
        pub fn lowest_set_bit(self) -> Option<usize> {
            if self.0 == 0 {
                // no bits have been set!
                None
            } else {
                Some(unsafe { self.lowest_set_bit_nonzero() })
            }
        }

        /// Returns the bitmask with the lowest bit removed. Pretty simple to understand:
        /// `011010` yields `011000`, i.e the lowest order bit is removed. We don't need
        /// to know the index; if we did, we could have done something like `bits ^= (1 << index)`
        pub fn remove_lowest_bit(self) -> Self {
            Bitmask(self.0 & (self.0 - 1))
        }

        /// Returns the first bit set in the bitmask, if such a bit exists
        ///
        /// Please check that atleast a single bit has been set in the bitmask before attempting
        /// to use this!
        pub unsafe fn lowest_set_bit_nonzero(self) -> usize {
            // we can use the cttz (count trailing zeros/ unset bits) intrinsic when it is stabilized
            self.trailing_zeros()
        }

        /// Checks if any bit has been set in the bitmask
        pub fn any_bit_set(self) -> bool {
            self.0 != 0
        }

        /// Returns the number of trailing zeros in this bitmask
        ///
        /// We just use or emulate the trailing zeros instructions, that is, by either reversing
        /// the byte order of the word directly, or by emulating it by swapping bytes.
        /// So if our bitmask is something like: 0b1111_1000, then we'd get 0000_0011 (base 10)
        pub fn trailing_zeros(self) -> usize {
            if cfg!(target_arch = "arm") && BITMASK_STRIDE % 8 == 0 {
                /*
                 ARM doesn't have a trailing zeros instruction and instead
                 we need to use a combination of RBIT (reverse bits) and then CLZ (count
                 leading zeros). However, even worse, pre-arm-v7 doesn't have RBIT (for more
                 information, see this:
                 https://developer.arm.com/documentation/dui0489/h/arm-and-thumb-instructions/rev--rev16--revsh--and-rbit)
                 That is why we'll swap bytes (basically 0x12345678 becomes 0x87654321)
                 and then get the leading zeros which effectively does the same thing
                */
                self.0.swap_bytes().leading_zeros() as usize / BITMASK_STRIDE
            } else {
                self.0.trailing_zeros() as usize / BITMASK_STRIDE
            }
        }

        /// Returns the number of leading zeros in the bitmask
        ///
        pub fn leading_zeros(self) -> usize {
            // Fortunately architectures do have the leading_zeros instruction :)
            // so we don't have to do some cfg mess
            self.0.leading_zeros() as usize / BITMASK_STRIDE
        }
    }

    /// An iterator over the contents of a bitmask, returning the indices
    /// of the set bits
    pub struct BitmaskIterator(Bitmask);

    impl Iterator for BitmaskIterator {
        type Item = usize;
        fn next(&mut self) -> Option<usize> {
            let bit = self.0.lowest_set_bit()?;
            self.0 = self.0.remove_lowest_bit();
            Some(bit)
        }
    }

    impl IntoIterator for Bitmask {
        type IntoIter = BitmaskIterator;
        type Item = usize;
        fn into_iter(self) -> Self::IntoIter {
            BitmaskIterator(self)
        }
    }
}

#[cfg(any(not(target_arch = "x86_64"), not(target_arch = "x86")))]
mod generic {
    //! Implementations for CPU architectures that do not support SSE instructions
    /*
        TODO(@ohsayan): Evaluate the need for NEON/AVX. Also note, SSE3/ SSE4 can
        prove to have much faster vector operations, but older CPUs may not support it (even worse,
        all those intrinsics are unstable on Rust, so that makes using them further problematic).
        Our job is to first build for SSE2 since that has the best support (all the way from Pentium
        chips). NEON has multi-cycle latencies, so that needs more evaluation.

        Note about the `GroupWord`s: we choose the target's pointer word width than just blindly
        using 64-bit pointer sizes because using 64-bit on 32-bit systems would only add to slowness
    */

    use super::bitmask::Bitmask;
    use super::control_bytes;
    use core::mem;
    use core::ptr;

    #[cfg(target_pointer_width = "64")]
    type GroupWord = u64;

    #[cfg(target_pointer_width = "32")]
    type GroupWord = u32;

    /// Just use the expected pointer width publicly for sanity
    pub type BitmaskWord = GroupWord;

    fn repeat(byte: u8) -> GroupWord {
        GroupWord::from_ne_bytes([byte; Group::WIDTH])
    }

    pub const BITMASK_STRIDE: usize = 8;
    pub const BITMASK_MASK: BitmaskWord = 0x8080_8080_8080_8080_u64 as BitmaskWord;

    /// A group of control-bytes that can be scanned in parallel
    pub struct Group(GroupWord);

    impl Group {
        /// This will return either 32/64 depending on the target's pointer width
        pub const WIDTH: usize = mem::size_of::<Self>();
        /// Returns a full group
        pub const fn empty_static() -> &'static [u8; Group::WIDTH] {
            #[repr(C)]
            struct AlignedBytes {
                // some explicit padding for alignment to ensure alignment to the group size
                _align: [Group; 0],
                bytes: [u8; Group::WIDTH],
            }
            const ALIGNED_BYTES: AlignedBytes = AlignedBytes {
                _align: [],
                bytes: [control_bytes::EMPTY; Group::WIDTH],
            };
            &ALIGNED_BYTES.bytes
        }

        /// Load a group of bytes starting at the provided address (unaligned read)
        pub unsafe fn load_unaligned(ptr: *const u8) -> Self {
            Group(ptr::read_unaligned(ptr.cast()))
        }

        /// Load a group of bytes starting at the provided address (aligned read)
        pub unsafe fn load_aligned(ptr: *const u8) -> Self {
            Group(ptr::read(ptr.cast()))
        }

        /// Store the [`Group`] in the given address. This is guaranteed to be aligned
        pub unsafe fn store_aligned(self, ptr: *mut u8) {
            ptr::write(ptr.cast(), self.0)
        }

        /// Returns a bitmask indicating which all bytes in the group _may_ have this value.
        /// This trick is derived from (the original site is inaccesible at times, for me at least):
        /// https://web.archive.org/web/20210523160500/http://graphics.stanford.edu/~seander/bithacks.html##ValueInWord.
        ///
        /// This however can return a false positive, but since after checking metadata for an entry,
        /// we _do_ check the value for equality, so this wouldn't cause anything to go wrong, fortunately.
        /// The drawback: a little loss on performance for the equality check in the case of a false positive,
        /// but this is extremely insignificant. This is something like C++'s
        /// [strchr](https://en.cppreference.com/w/c/string/byte/strchr)
        pub fn match_byte(self, byte: u8) -> Bitmask {
            let cmp = self.0 ^ repeat(byte);
            // change to little endian
            Bitmask((cmp.wrapping_sub(repeat(0x01)) & !cmp & repeat(0x80)).to_le())
        }

        /// Returns a bitmask indicating which all bytes were empty
        pub fn match_empty(self) -> Bitmask {
            // always change to little endian
            Bitmask((self.0 & (self.0 << 1)) & repeat(0x80).to_le())
        }

        /// Returns a bitmask indicating which all bytes were empty or deleted
        pub fn match_empty_or_deleted(self) -> Bitmask {
            // A byte is EMPTY or DELETED iff the high bit is set
            Bitmask((self.0 & repeat(0x80)).to_le())
        }

        /// Returns a bitmask indicating which all bytes were full
        pub fn match_full(self) -> Bitmask {
            self.match_empty_or_deleted().invert()
        }

        /// Transform DELETED => EMPTY, EMPTY => EMPTY (specials) and FULL => DELETED
        pub fn transform_full_to_deleted_and_special_to_empty(self) -> Self {
            /*
             If high order bit is 1 => EMPTY or DELETED => is special
             If high order bit is 0 => FULL => not special

             So we do this manually unlike SSE2 that does it for us:
             1. !0b1000_0000 => 1111111101111111
             2. 1111111101111111 + 1 => (11111111)10000000 (after shl 7)
             Similarly,
             1. !0b0000_0000 => 1111111111111111
             2. 1111111111111111 + 0 => (11111111)11111111 (after shl 7)
            */
            let full = !self.0 & repeat(0x80);
            Group(!full + (full >> 7))
        }
    }
}

mod mapalloc {
    //! Primitive methods for allocation
    use core::alloc::Layout;
    use core::ptr::NonNull;
    use std::alloc;

    /// This trait defines an allocator. The reason we don't directly use the host allocator
    /// and abstract it away with a trait is for future events when we may build our own
    /// allocator (or maybe support embedded!? gosh, that'll be some task)
    pub unsafe trait Allocator {
        fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, ()> {
            unsafe { NonNull::new(alloc::alloc(layout)).ok_or(()) }
        }
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            alloc::dealloc(ptr.as_ptr(), layout)
        }
    }

    pub struct Global;
    impl Default for Global {
        fn default() -> Self {
            Global
        }
    }

    /// Use a given allocator `A` to allocate for a given memory layout
    pub fn self_allocate<A: Allocator>(allocator: &A, layout: Layout) -> Result<NonNull<u8>, ()> {
        allocator.allocate(layout)
    }
}

mod control_bytes {
    /// Control byte value for an empty bucket.
    pub const EMPTY: u8 = 0b1111_1111;
}