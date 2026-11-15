//! Files mapped shared. While any mapping shares a file, the file's first pages live in a block of
//! shared pages that every such mapping shows, and the file's reads and writes reach the same block, so
//! all see the same bytes as in Linux's page cache.

use alloc::rc::Rc;
use wcontainers_abi::{MemoryId, Permissions};

use crate::fs::{Block, Content, Node};
use crate::memory::{Backing, page_up};
use crate::{Context, Errno, Kernel, errno};

/// Makes `length` bytes at `address` of a memory show the file's pages from `offset`, giving the file
/// a block, or a larger one, if it needs it.
pub(crate) fn attach(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    memory: MemoryId,
    address: u64,
    offset: u64,
    length: u64,
) -> Result<(), Errno> {
    let needed = offset + length;
    let old = match &node.borrow().content {
        Content::Regular(regular) => regular.block,
        _ => return Ok(()),
    };
    if old.is_none_or(|old| old.length < needed) {
        grow(kernel, cx, node, old, needed)?;
    }
    let block = match &node.borrow().content {
        Content::Regular(regular) => regular.block.expect("the file has a block"),
        _ => unreachable!("the file is regular"),
    };
    cx.cpu
        .shared_map(block.id, offset, memory, address, length)
        .map_err(errno::from_cpu)
}

/// Gives the file a block of `length` bytes holding what it held, and moves every mapping that showed
/// the old block over to it.
fn grow(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    old: Option<Block>,
    length: u64,
) -> Result<(), Errno> {
    let id = cx.cpu.shared_new(length).map_err(errno::from_cpu)?;
    let new = Block { id, length };
    {
        let mut inode = node.borrow_mut();
        let Content::Regular(regular) = &mut inode.content else {
            unreachable!("the file is regular");
        };
        regular.take_block(cx.cpu);
        let bytes = regular.read(cx.cpu, 0, length);
        cx.cpu
            .shared_write(id, 0, &bytes)
            .map_err(errno::from_cpu)?;
        regular.block = Some(new);
    }
    match old {
        Some(old) => {
            for process in kernel.processes.values() {
                for mapping in process.map.iter() {
                    if let Some(offset) = shared_offset(&mapping.backing, mapping.shared, node) {
                        cx.cpu
                            .shared_map(
                                id,
                                offset,
                                process.memory,
                                mapping.start,
                                mapping.end - mapping.start,
                            )
                            .map_err(errno::from_cpu)?;
                    }
                }
            }
            cx.cpu.shared_drop(old.id);
        }
        None => kernel.files.shared.push(node.clone()),
    }
    Ok(())
}

/// Whether a shared mapping of the file may write it, now or after `mprotect` (Linux's
/// `mapping_writably_mapped`).
pub(crate) fn writably_mapped(kernel: &Kernel, node: &Node) -> bool {
    kernel.processes.values().any(|process| {
        process.map.iter().any(|mapping| {
            mapping.allowed.contains(Permissions::WRITE)
                && shared_offset(&mapping.backing, mapping.shared, node).is_some()
        })
    })
}

/// The file offset a mapping starts at, if it shares `node`.
fn shared_offset(backing: &Backing, shared: bool, node: &Node) -> Option<u64> {
    match backing {
        Backing::File {
            node: mapped,
            offset,
            ..
        } if shared && Rc::ptr_eq(mapped, node) => Some(*offset),
        _ => None,
    }
}

/// After a file's size changed: the pages of shared mappings wholly past its end fault with `SIGBUS`,
/// and those before it are reachable again.
pub(crate) fn resized(kernel: &mut Kernel, cx: &mut Context<'_>, node: &Node) -> Result<(), Errno> {
    let size = match &node.borrow().content {
        Content::Regular(regular) if regular.block.is_some() => regular.size(),
        _ => return Ok(()),
    };
    for process in kernel.processes.values_mut() {
        process
            .map
            .set_file_end(cx.cpu, process.memory, node, page_up(size))?;
    }
    Ok(())
}

/// Ends the blocks of files no mapping shares any more, moving their bytes back to the files.
pub(crate) fn release(kernel: &mut Kernel, cx: &mut Context<'_>) {
    let processes = &kernel.processes;
    kernel.files.shared.retain(|node| {
        let mapped = processes.values().any(|process| {
            process
                .map
                .iter()
                .any(|mapping| shared_offset(&mapping.backing, mapping.shared, node).is_some())
        });
        if !mapped {
            let mut inode = node.borrow_mut();
            if let Content::Regular(regular) = &mut inode.content
                && let Some(block) = regular.take_block(cx.cpu)
            {
                cx.cpu.shared_drop(block.id);
            }
        }
        mapped
    });
}
