// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io::{self, Read, Write};

use crate::encoding::Marshallable;

const DEFAULT_ALLOCATION_SIZE: usize = 64 * 1024; // 64 KiB

#[derive(Debug, PartialEq)]
pub(crate) struct BroadcastPayload {
    pub(crate) height: u8,
    pub(crate) gossip_frame: Vec<u8>,
}

impl Marshallable for BroadcastPayload {
    fn marshal_binary<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&[self.height])?;
        let len = self.gossip_frame.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&self.gossip_frame)?;
        Ok(())
    }
    fn unmarshal_binary<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut height_buf = [0; 1];
        reader.read_exact(&mut height_buf)?;
        let mut gossip_length_buf = [0; 4];
        reader.read_exact(&mut gossip_length_buf)?;
        let gossip_length = u32::from_le_bytes(gossip_length_buf) as usize;

        // To prevent inefficient memory usage, we read the gossip frame in chunks if its length exceeds a reasonable threshold.
        let gossip_frame = if gossip_length > DEFAULT_ALLOCATION_SIZE {
            let mut gossip_frame = Vec::with_capacity(DEFAULT_ALLOCATION_SIZE);
            let mut bytes_left = gossip_length;
            let mut buffer = [0u8; DEFAULT_ALLOCATION_SIZE];
            while bytes_left > 0 {
                let to_read = bytes_left.min(buffer.len());
                reader.read_exact(&mut buffer[..to_read])?;
                gossip_frame.extend_from_slice(&buffer[..to_read]);
                bytes_left -= to_read;
            }
            gossip_frame
        } else {
            let mut gossip_frame = vec![0; gossip_length];
            reader.read_exact(&mut gossip_frame)?;
            gossip_frame
        };

        Ok(BroadcastPayload {
            height: height_buf[0],
            gossip_frame,
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};

    /// Global allocator wrapper that panics when allocating too much memory.
    struct MemoryCapAllocator;

    const LOW_MEMORY_THRESHOLD: usize = 16 * 1024 * 1024; // 16 MiB

    unsafe impl GlobalAlloc for MemoryCapAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if layout.size() > LOW_MEMORY_THRESHOLD {
                panic!(
                    "unexpected huge allocation: {} bytes (threshold {})",
                    layout.size(),
                    LOW_MEMORY_THRESHOLD
                );
            }
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static A: MemoryCapAllocator = MemoryCapAllocator;

    #[test]
    fn unmarshal_invalid_gossip_length() {
        // height=0, gossip_length=0xFFFF_FFFF, and no gossip bytes after that.
        // Correct behavior: return UnexpectedEof.
        let mut bytes = Vec::new();
        bytes.push(0u8);
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());

        let res = BroadcastPayload::unmarshal_binary(&mut &bytes[..]);

        let err =
            res.expect_err("expected an error due to invalid gossip length");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }
}
