//! Files mapped into memory. While any mapping shows a file, the file's first pages live in a block of
//! shared pages that every such mapping shows (a private one except where it holds its own copies),
//! and the file's reads and writes reach the same block, so all see the same bytes as in Linux's page
//! cache.

use alloc::rc::Rc;
use wcontainers_abi::{MemoryId, Permissions};

use crate::fs::{Block, Content, Node};
use crate::memory::{Backing, Mapping, page_up};
use crate::{Context, Errno, Kernel, errno};

/// The file's block, made or grown so it holds at least the first `length` bytes; `None` for a file
/// that is not regular.
pub(crate) fn block_for(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    length: u64,
) -> Result<Option<Block>, Errno> {
    let old = match &node.borrow().content {
        Content::Regular(regular) => regular.block,
        _ => return Ok(None),
    };
    if old.is_none_or(|old| old.length < length) {
        grow(kernel, cx, node, old, length)?;
    }
    Ok(match &node.borrow().content {
        Content::Regular(regular) => regular.block,
        _ => unreachable!("the file is regular"),
    })
}

/// Makes a file mapping show its file's block, except at a private mapping's own copies.
pub(crate) fn show(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    memory: MemoryId,
    mapping: &Mapping,
) -> Result<(), Errno> {
    let Backing::File { node, offset, .. } = &mapping.backing else {
        return Ok(());
    };
    let Some(block) = block_for(kernel, cx, node, offset + (mapping.end - mapping.start))? else {
        return Ok(());
    };
    map_shown(cx, block, memory, mapping, *offset)
}

/// Makes every file mapping of a memory not yet in any process show its file: every block is made
/// big enough first, so none grows while another mapping already shows it.
pub(crate) fn show_all(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    memory: MemoryId,
    map: &crate::memory::AddressMap,
) -> Result<(), Errno> {
    for mapping in map.iter() {
        if let Backing::File { node, offset, .. } = &mapping.backing {
            block_for(kernel, cx, node, offset + (mapping.end - mapping.start))?;
        }
    }
    for mapping in map.iter() {
        show(kernel, cx, memory, mapping)?;
    }
    Ok(())
}

/// Maps the block over the parts of `mapping` that show its file.
fn map_shown(
    cx: &mut Context<'_>,
    block: Block,
    memory: MemoryId,
    mapping: &Mapping,
    offset: u64,
) -> Result<(), Errno> {
    for (from, to, own) in mapping.runs() {
        if mapping.shared || !own {
            cx.cpu
                .shared_map(
                    block.id,
                    offset + (from - mapping.start),
                    memory,
                    from,
                    to - from,
                )
                .map_err(|error| cx.cpu_error(error))?;
        }
    }
    Ok(())
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
    let id = cx
        .cpu
        .shared_new(length)
        .map_err(|error| cx.cpu_error(error))?;
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
                for mapping in process.map.borrow().iter() {
                    if let Some(offset) = mapped_offset(&mapping.backing, node) {
                        map_shown(cx, new, process.memory, mapping, offset)?;
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
        process.map.borrow().iter().any(|mapping| {
            mapping.allowed.contains(Permissions::WRITE)
                && mapping.shared
                && mapped_offset(&mapping.backing, node).is_some()
        })
    })
}

/// The file offset a mapping starts at, if it maps `node`.
fn mapped_offset(backing: &Backing, node: &Node) -> Option<u64> {
    match backing {
        Backing::File {
            node: mapped,
            offset,
            ..
        } if Rc::ptr_eq(mapped, node) => Some(*offset),
        _ => None,
    }
}

/// After a file's bytes or size changed: the pages of its mappings wholly past its end fault with
/// `SIGBUS`, those before it are reachable again, and code run from it is stale.
pub(crate) fn changed(kernel: &mut Kernel, cx: &mut Context<'_>, node: &Node) -> Result<(), Errno> {
    let size = match &node.borrow().content {
        Content::Regular(regular) if regular.block.is_some() => regular.size(),
        _ => return Ok(()),
    };
    for process in kernel.processes.values_mut() {
        process
            .map
            .borrow_mut()
            .set_file_end(cx.cpu, process.memory, node, page_up(size))?;
        for mapping in process.map.borrow().iter() {
            if mapping.permissions.contains(Permissions::EXECUTE)
                && mapped_offset(&mapping.backing, node).is_some()
            {
                cx.cpu.code_changed(
                    process.memory,
                    mapping.start,
                    mapping.end - mapping.start,
                    &[],
                );
            }
        }
    }
    Ok(())
}

/// Ends the blocks of files no mapping shows any more, moving their bytes back to the files.
pub(crate) fn release(kernel: &mut Kernel, cx: &mut Context<'_>) {
    let processes = &kernel.processes;
    kernel.files.shared.retain(|node| {
        let mapped = processes.values().any(|process| {
            process
                .map
                .borrow_mut()
                .iter()
                .any(|mapping| mapped_offset(&mapping.backing, node).is_some())
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
