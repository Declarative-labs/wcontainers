//! Files mapped into memory. While any mapping shows a file, the file's first pages live in a block of
//! shared pages that every such mapping shows (a private one except where it holds its own copies),
//! and the file's reads and writes reach the same block, so all see the same bytes as in Linux's page
//! cache. A backend file's block is read from the backend before a mapping needs it
//! ([`backend_block`]) and written back while a shared mapping may write it.

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use wcontainers_abi::{MemoryId, Permissions};

use crate::fs::backend::{Handle, Pages};
use crate::fs::{Block, Content, Node};
use crate::memory::{Backing, Mapping, page_up};
use crate::{Context, Errno, Kernel, errno};

/// The file's block, made or grown so it holds at least the first `length` bytes; `None` for a file
/// that is neither regular nor on the backend. A backend file's block was made big enough by
/// [`backend_block`] first, since its bytes come from the backend.
pub(crate) fn block_for(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    length: u64,
) -> Result<Option<Block>, Errno> {
    let old = match &node.borrow().content {
        Content::Regular(regular) => regular.block,
        Content::Backend(remote) => {
            return match &remote.pages {
                Some(pages) if pages.block.length >= length => Ok(Some(pages.block)),
                _ => Err(Errno::EIO),
            };
        }
        _ => return Ok(None),
    };
    if old.is_none_or(|old| old.length < length) {
        grow(kernel, cx, node, old, length)?;
    }
    Ok(node.borrow().content.block())
}

/// Makes a backend file's block hold at least its first `length` bytes, reading what it lacks
/// through `handle` (or the open file it was read through), which parks the call until the backend
/// answers. A shared mapping that may write (`writer`) has its file write the pages back.
pub(crate) fn backend_block(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    handle: Option<&Rc<Handle>>,
    length: u64,
    writer: bool,
) -> Result<(), Errno> {
    let (old, through) = match &node.borrow().content {
        Content::Backend(remote) => match &remote.pages {
            Some(pages) => (Some(pages.block), handle.unwrap_or(&pages.handle).clone()),
            None => (None, handle.ok_or(Errno::EIO)?.clone()),
        },
        _ => return Err(Errno::EIO),
    };
    let have = old.map_or(0, |old| old.length);
    if have < length {
        let bytes = crate::fs::backend::read(&through, have, length - have)?;
        kernel.counters.count(
            cx.host,
            crate::reason::BACKEND_READ_WHOLE,
            bytes.len() as u64,
        );
        backend_grow(kernel, cx, node, old, &through, length, &bytes)?;
    }
    if writer
        && let Some(handle) = handle
        && let Content::Backend(remote) = &mut node.borrow_mut().content
        && let Some(pages) = &mut remote.pages
    {
        pages.handle = handle.clone();
        pages.written = true;
    }
    Ok(())
}

/// Makes a backend file's block hold its first pages from `bytes`, the whole file as just read
/// through `handle`, unless it holds them already.
pub(crate) fn backend_block_of(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    handle: &Rc<Handle>,
    bytes: &[u8],
) -> Result<(), Errno> {
    let old = node.borrow().content.block();
    let have = old.map_or(0, |old| old.length);
    let length = page_up(bytes.len() as u64);
    kernel.counters.count(
        cx.host,
        crate::reason::BACKEND_READ_WHOLE,
        bytes.len() as u64,
    );
    if have < length {
        let tail = &bytes[(have as usize).min(bytes.len())..];
        backend_grow(kernel, cx, node, old, handle, length, tail)?;
    }
    Ok(())
}

/// Gives a backend file a block of `length` bytes: what its old one held, then `tail`, then zeros.
fn backend_grow(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    old: Option<Block>,
    through: &Rc<Handle>,
    length: u64,
    bytes: &[u8],
) -> Result<(), Errno> {
    let have = old.map_or(0, |old| old.length);
    {
        let id = cx
            .cpu
            .shared_new(length)
            .map_err(|error| cx.cpu_error(error))?;
        let mut held = vec![0; have as usize];
        if let Some(old) = old {
            cx.cpu
                .shared_read(old.id, 0, &mut held)
                .map_err(errno::from_cpu)?;
        }
        held.extend_from_slice(bytes);
        held.truncate(length as usize);
        cx.cpu.shared_write(id, 0, &held).map_err(errno::from_cpu)?;
        let new = Block { id, length };
        if let Content::Backend(remote) = &mut node.borrow_mut().content {
            match &mut remote.pages {
                Some(pages) => pages.block = new,
                None => {
                    remote.pages = Some(Pages {
                        block: new,
                        handle: through.clone(),
                        written: false,
                    });
                }
            }
        }
        moved(kernel, cx, node, old, new)
    }
}

/// The bytes a mapped backend file's block holds up to `size` from `offset`, or `None` while
/// nothing maps it.
pub(crate) fn backend_bytes(
    cx: &mut Context<'_>,
    node: &Node,
    offset: u64,
    count: u64,
    size: u64,
) -> Option<Vec<u8>> {
    let block = node.borrow().content.block()?;
    let end = offset.saturating_add(count).min(size).min(block.length);
    let mut bytes = vec![0; end.saturating_sub(offset) as usize];
    cx.cpu
        .shared_read(block.id, offset, &mut bytes)
        .expect("the block holds the file's first pages");
    Some(bytes)
}

/// A read of a backend file: from its block while it is mapped, as from Linux's page cache, with
/// the size asked again first (`fuse_cache_read_iter` with FUSE_AUTO_INVAL_DATA) and what lies past
/// the block read from the backend; from the backend alone when the backend opened the file for
/// direct reads.
pub(crate) fn backend_read(
    cx: &mut Context<'_>,
    node: &Node,
    handle: &Handle,
    offset: u64,
    count: u64,
) -> Result<Vec<u8>, Errno> {
    let Some(block) = node
        .borrow()
        .content
        .block()
        .filter(|_| !handle.direct_io())
    else {
        return crate::fs::backend::read(handle, offset, count);
    };
    crate::fs::backend::getattr(node, Some(handle.fh))?;
    let size = node.borrow().size();
    let mut bytes = backend_bytes(cx, node, offset, count, size).unwrap_or_default();
    let at = offset + bytes.len() as u64;
    if at >= block.length && at < size && (bytes.len() as u64) < count {
        bytes.extend(crate::fs::backend::read(
            handle,
            at,
            count - bytes.len() as u64,
        )?);
    }
    Ok(bytes)
}

/// Puts bytes written to a backend file into its block, where it has one.
pub(crate) fn backend_written(cx: &mut Context<'_>, node: &Node, offset: u64, bytes: &[u8]) {
    if let Some(block) = node.borrow().content.block()
        && offset < block.length
    {
        let end = (offset + bytes.len() as u64).min(block.length);
        cx.cpu
            .shared_write(block.id, offset, &bytes[..(end - offset) as usize])
            .expect("the block holds the file's first pages");
    }
}

/// Zeroes `[from, to)` of a backend file's block, for bytes the backend dropped (a truncation or a
/// punched hole), as Linux's `truncate_pagecache_range`.
pub(crate) fn backend_cut(cx: &mut Context<'_>, node: &Node, from: u64, to: u64) {
    if let Some(block) = node.borrow().content.block()
        && from < block.length
    {
        let zeros = vec![0; (to.min(block.length) - from) as usize];
        cx.cpu
            .shared_write(block.id, from, &zeros)
            .expect("the block holds the file's first pages");
    }
}

/// Writes a backend file's pages in `[from, to)` back, up to its end, if a shared mapping may have
/// written them: waiting for the answers, or with nothing to wait sending them on their own. Every
/// page goes, changed or not, since which were written is not known.
pub(crate) fn backend_write_back(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    from: u64,
    to: u64,
    wait: bool,
) -> Result<(), Errno> {
    let (handle, size) = match &node.borrow().content {
        Content::Backend(remote) => match &remote.pages {
            Some(pages) if pages.written => (pages.handle.clone(), remote.size.get()),
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    let Some(bytes) = backend_bytes(cx, node, from, to.saturating_sub(from), size) else {
        return Ok(());
    };
    if bytes.is_empty() {
        return Ok(());
    }
    crate::fs::backend::write_back(&handle, from, &bytes, wait)?;
    kernel
        .counters
        .count(cx.host, crate::reason::BACKEND_PAGES_WRITTEN_WHOLE, 0);
    Ok(())
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
    moved(kernel, cx, node, old, new)
}

/// Moves every mapping that showed the file's old block over to its new one and drops the old; a
/// file's first block puts it on the list of mapped files.
fn moved(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    node: &Node,
    old: Option<Block>,
    new: Block,
) -> Result<(), Errno> {
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
        Content::Backend(remote) if remote.pages.is_some() => remote.size.get(),
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

/// Ends the blocks of files no mapping shows any more, moving their bytes back to the files: a
/// backend file's are written back, with nothing waiting, if a shared mapping may have written them.
pub(crate) fn release(kernel: &mut Kernel, cx: &mut Context<'_>) {
    let processes = &kernel.processes;
    let mut unmapped = Vec::new();
    kernel.files.shared.retain(|node| {
        let mapped = processes.values().any(|process| {
            process
                .map
                .borrow_mut()
                .iter()
                .any(|mapping| mapped_offset(&mapping.backing, node).is_some())
        });
        if !mapped {
            unmapped.push(node.clone());
        }
        mapped
    });
    for node in unmapped {
        let _ = backend_write_back(kernel, cx, &node, 0, u64::MAX, false);
        let mut inode = node.borrow_mut();
        let block = match &mut inode.content {
            Content::Regular(regular) => regular.take_block(cx.cpu),
            Content::Backend(remote) => remote.pages.take().map(|pages| pages.block),
            _ => None,
        };
        if let Some(block) = block {
            cx.cpu.shared_drop(block.id);
        }
    }
}
