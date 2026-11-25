//! Files mapped into memory. While any mapping shows a file, the file's first pages live in a block of
//! shared pages that every such mapping shows (a private one except where it holds its own copies),
//! and the file's reads and writes reach the same block, so all see the same bytes as in Linux's page
//! cache. A backend file's block takes each page from the backend when it is first touched
//! ([`fill`]) and is written back while a shared mapping may write it.

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use wcontainers_abi::{MemoryId, PAGE_SIZE, Permissions};

use crate::fs::backend::{Handle, Pages};
use crate::fs::{Block, Content, Node};
use crate::memory::{Backing, Mapping, page_up};
use crate::{Context, Errno, Kernel, errno};

/// The file's block, made or grown so it holds at least the first `length` bytes; `None` for a file
/// that is neither regular nor on the backend. A backend file's block was made big enough by
/// [`backend_block`] first, since its bytes come from the backend.
pub(crate) fn block_for<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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

/// Makes a backend file's block cover at least its first `length` bytes, its pages read only when
/// touched ([`fill`]), through `handle` or the open file it was mapped through. A shared mapping
/// that may write (`writer`) has its file write the pages back.
pub(crate) fn backend_block<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    node: &Node,
    handle: Option<&Arc<Handle>>,
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
    if old.map_or(0, |old| old.length) < length {
        backend_grow(kernel, cx, node, old, &through, page_up(length))?;
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

/// Gives a backend file a block of `length` bytes holding the pages its old one held.
fn backend_grow<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    node: &Node,
    old: Option<Block>,
    through: &Arc<Handle>,
    length: u64,
) -> Result<(), Errno> {
    let id = cx
        .cpu
        .shared_new(length)
        .map_err(|error| cx.cpu_error(error))?;
    let new = Block { id, length };
    if let Content::Backend(remote) = &mut node.borrow_mut().content {
        match &mut remote.pages {
            Some(pages) => {
                for run in pages.filled.iter() {
                    let mut held = vec![0; ((run.end - run.start) * PAGE_SIZE) as usize];
                    cx.cpu
                        .shared_read(pages.block.id, run.start * PAGE_SIZE, &mut held)
                        .map_err(errno::from_cpu)?;
                    cx.cpu
                        .shared_write(id, run.start * PAGE_SIZE, &held)
                        .map_err(errno::from_cpu)?;
                }
                pages.block = new;
            }
            None => {
                remote.pages = Some(Pages {
                    block: new,
                    handle: through.clone(),
                    written: false,
                    dirtied: false,
                    sent: crossbeam_utils::atomic::AtomicCell::new(0),
                    sent_at_attributes: 0,
                    filled: rangemap::RangeSet::new(),
                    failed: rangemap::RangeSet::new(),
                    changes: 0,
                });
            }
        }
    }
    moved(kernel, cx, node, old, new)
}

/// The runs of file pages `pages` that a mapped backend file's block lacks; none for another file.
pub(crate) fn unfilled(node: &Node, pages: Range<u64>) -> Vec<Range<u64>> {
    if pages.start >= pages.end {
        return Vec::new();
    }
    match &node.borrow().content {
        Content::Backend(remote) => match &remote.pages {
            Some(held) => held.filled.gaps(&pages).collect(),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// How many pages a touch of a mapped backend file reads around it (Linux's `ra_pages`).
pub(crate) fn readahead_pages(node: &Node) -> u64 {
    crate::fs::backend::remote(node)
        .map_or(1, |(client, _)| client.settled().readahead_pages.max(1))
}

/// Makes a mapped backend file's block hold its file pages `pages` (those within the block):
/// pages wholly past the file's end are zero, the rest are asked for, as from the thread whose
/// call or fault runs. `PARKED` while any is awaited; EFAULT for a page whose read failed (once,
/// so the next touch reads again) or when nothing can ask.
pub(crate) fn fill(node: &Node, pages: Range<u64>) -> Result<(), Errno> {
    let (client, handle, gaps, changes) = {
        let mut inode = node.borrow_mut();
        let Content::Backend(remote) = &mut inode.content else {
            return Ok(());
        };
        let size = remote.size.load();
        let Some(held) = &mut remote.pages else {
            return Ok(());
        };
        let range = pages.start..pages.end.min(held.block.length / PAGE_SIZE);
        if range.start >= range.end {
            return Ok(());
        }
        let file_pages = page_up(size) / PAGE_SIZE;
        if file_pages < range.end {
            held.filled.insert(file_pages.max(range.start)..range.end);
        }
        if held.failed.overlaps(&range) {
            held.failed.remove(range);
            return Err(Errno::EFAULT);
        }
        let gaps: Vec<Range<u64>> = held.filled.gaps(&range).collect();
        if gaps.is_empty() {
            return Ok(());
        }
        (
            remote.client.clone(),
            held.handle.clone(),
            gaps,
            held.changes,
        )
    };
    for gap in gaps {
        if !client.ask_pages(node, &handle, gap, changes) {
            return Err(Errno::EFAULT);
        }
    }
    Err(Errno::PARKED)
}

/// The backend answered for file pages `pages` asked when the file had had `changes` changes:
/// the pages its block still lacks take the bytes (zeros past what came), unless the file changed
/// since; a failed read marks them so their next touch faults.
pub(crate) fn pages_came<S: crate::System>(
    cx: &mut Context<'_, S>,
    node: &Node,
    pages: Range<u64>,
    changes: u64,
    result: Result<crate::fs::backend::Body, Errno>,
) {
    let mut inode = node.borrow_mut();
    let Content::Backend(remote) = &mut inode.content else {
        return;
    };
    let Some(held) = &mut remote.pages else {
        return;
    };
    let range = pages.start..pages.end.min(held.block.length / PAGE_SIZE);
    if range.start >= range.end || held.changes != changes {
        return;
    }
    let gaps: Vec<Range<u64>> = held.filled.gaps(&range).collect();
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(_) => {
            for gap in gaps {
                held.failed.insert(gap);
            }
            return;
        }
    };
    for gap in gaps {
        let from = ((gap.start - pages.start) * PAGE_SIZE) as usize;
        let to = ((gap.end - pages.start) * PAGE_SIZE) as usize;
        let mut chunk = vec![0; to - from];
        if from < bytes.len() {
            let came = &bytes[from..to.min(bytes.len())];
            chunk[..came.len()].copy_from_slice(came);
        }
        cx.cpu
            .shared_write(held.block.id, gap.start * PAGE_SIZE, &chunk)
            .expect("the block holds the file's first pages");
        held.filled.insert(gap);
    }
}

/// `[from, to)` of a mapped backend file in runs, each with whether its block holds it; `None`
/// while nothing maps the file. Runs end at the block's end; what lies past it is not held.
fn held_runs(node: &Node, from: u64, to: u64) -> Option<Vec<(u64, u64, bool)>> {
    let inode = node.borrow();
    let Content::Backend(remote) = &inode.content else {
        return None;
    };
    let held = remote.pages.as_ref()?;
    let mut runs = Vec::new();
    let end = to.min(held.block.length);
    if from < end {
        let pages = from / PAGE_SIZE..page_up(end) / PAGE_SIZE;
        let mut parts: Vec<(Range<u64>, bool)> = held
            .filled
            .overlapping(&pages)
            .map(|run| (run.clone(), true))
            .chain(held.filled.gaps(&pages).map(|gap| (gap, false)))
            .collect();
        parts.sort_by_key(|(run, _)| run.start);
        for (run, filled) in parts {
            let start = (run.start * PAGE_SIZE).max(from);
            let stop = (run.end * PAGE_SIZE).min(end);
            if start < stop {
                runs.push((start, stop, filled));
            }
        }
    }
    if end.max(from) < to {
        runs.push((end.max(from), to, false));
    }
    Some(runs)
}

/// The bytes `[from, to)` of a block.
fn block_bytes<S: crate::System>(
    cx: &mut Context<'_, S>,
    block: Block,
    from: u64,
    to: u64,
) -> Vec<u8> {
    let mut bytes = vec![0; (to - from) as usize];
    cx.cpu
        .shared_read(block.id, from, &mut bytes)
        .expect("the block holds the file's first pages");
    bytes
}

/// A read of a backend file: while it is mapped, the pages its block holds come from there, as
/// from Linux's page cache, with the size asked again first (`fuse_cache_read_iter` with
/// FUSE_AUTO_INVAL_DATA), and the rest from the backend; from the backend alone when the backend
/// opened the file for direct reads.
pub(crate) fn backend_read<S: crate::System>(
    cx: &mut Context<'_, S>,
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
    let end = offset.saturating_add(count).min(size);
    let runs = held_runs(node, offset, end.max(offset)).unwrap_or_default();
    let mut bytes = Vec::new();
    for (from, to, held) in runs {
        if held {
            bytes.extend(block_bytes(cx, block, from, to));
        } else {
            let came = crate::fs::backend::read(handle, from, to - from)?;
            let short = (came.len() as u64) < to - from;
            bytes.extend(came);
            if short {
                break;
            }
        }
    }
    Ok(bytes)
}

/// Puts bytes written to a backend file into the pages its block holds; the others are read
/// afresh when touched.
pub(crate) fn backend_written<S: crate::System>(
    cx: &mut Context<'_, S>,
    node: &Node,
    offset: u64,
    bytes: &[u8],
) {
    let end = offset + bytes.len() as u64;
    let Some(runs) = held_runs(node, offset, end) else {
        return;
    };
    let block = node.borrow().content.block().expect("a mapped file");
    for (from, to, held) in runs {
        if held {
            cx.cpu
                .shared_write(
                    block.id,
                    from,
                    &bytes[(from - offset) as usize..(to - offset) as usize],
                )
                .expect("the block holds the file's first pages");
        }
    }
    changed_bytes(node);
}

/// Bytes the backend gave to keep (`fuse_notify_store`): they go into the file's block, and the
/// pages they cover whole, or up to the file's end, are held from then on.
pub(crate) fn backend_stored<S: crate::System>(
    cx: &mut Context<'_, S>,
    node: &Node,
    offset: u64,
    bytes: &[u8],
) {
    let mut inode = node.borrow_mut();
    let Content::Backend(remote) = &mut inode.content else {
        return;
    };
    let size = remote.size.load();
    let Some(held) = &mut remote.pages else {
        return;
    };
    let end = (offset + bytes.len() as u64).min(held.block.length);
    if offset >= end {
        return;
    }
    cx.cpu
        .shared_write(held.block.id, offset, &bytes[..(end - offset) as usize])
        .expect("the block holds the file's first pages");
    let first = page_up(offset) / PAGE_SIZE;
    let last = if end >= size { page_up(end) } else { end } / PAGE_SIZE;
    if first < last {
        held.filled.insert(first..last);
    }
}

/// The bytes a mapped backend file's block holds from `offset`, up to `count` and the file's end,
/// stopping at the first page it lacks (`fuse_retrieve`); `None` while nothing maps the file.
pub(crate) fn backend_held<S: crate::System>(
    cx: &mut Context<'_, S>,
    node: &Node,
    offset: u64,
    count: u64,
) -> Option<Vec<u8>> {
    let size = node.borrow().size();
    let block = node.borrow().content.block()?;
    let runs = held_runs(
        node,
        offset,
        offset.saturating_add(count).min(size).max(offset),
    )?;
    let mut bytes = Vec::new();
    for (from, to, held) in runs {
        if !held {
            break;
        }
        bytes.extend(block_bytes(cx, block, from, to));
    }
    Some(bytes)
}

/// Counts a change to a mapped backend file's bytes, so answers asked before it are not taken.
fn changed_bytes(node: &Node) {
    if let Content::Backend(remote) = &mut node.borrow_mut().content
        && let Some(held) = &mut remote.pages
    {
        held.changes += 1;
    }
}

/// Drops the file pages `pages` a mapped backend file's block holds, so each is read again when
/// next touched (`invalidate_inode_pages2_range`), unless a shared mapping may have written the
/// block: which pages were written is not known, so all are kept ([`backend_dropped`] counts it).
pub(crate) fn backend_drop(node: &Node, pages: Range<u64>) {
    if let Content::Backend(remote) = &mut node.borrow_mut().content
        && let Some(held) = &mut remote.pages
        && !held.dirtied
    {
        held.filled.remove(pages.clone());
        held.failed.remove(pages);
        held.changes += 1;
    }
}

/// After [`backend_drop`]: every mapping of the file faults on the pages dropped, and code run from
/// it is stale; counted when the pages were kept.
pub(crate) fn backend_dropped<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    node: &Node,
) -> Result<(), Errno> {
    let kept = match &node.borrow().content {
        Content::Backend(remote) => remote.pages.as_ref().map(|held| held.dirtied),
        _ => None,
    };
    match kept {
        None => return Ok(()),
        Some(true) => {
            let ino = node.borrow().ino;
            kernel
                .counters
                .count(cx.host, crate::reason::BACKEND_CHANGE_NOT_MAPPED, ino);
        }
        Some(false) => {
            for process in kernel.processes.values() {
                let mut map = process.map.borrow_mut();
                let shown: Vec<Range<u64>> = map
                    .iter()
                    .filter(|mapping| mapped_offset(&mapping.backing, node).is_some())
                    .map(|mapping| mapping.start..mapping.end)
                    .collect();
                for range in shown {
                    map.refresh(cx.cpu, process.memory, range.start, range.end)?;
                }
            }
        }
    }
    changed(kernel, cx, node)
}

/// A shared mapping can write the backend file `node` maps: its pages may hold bytes the backend
/// lacks from now on.
pub(crate) fn backend_writable(node: &Node) {
    if let Content::Backend(remote) = &mut node.borrow_mut().content
        && let Some(held) = &mut remote.pages
    {
        held.dirtied = true;
    }
}

/// Zeroes `[from, to)` of a backend file's block, for bytes the backend dropped (a truncation or a
/// punched hole), as Linux's `truncate_pagecache_range`.
pub(crate) fn backend_cut<S: crate::System>(
    cx: &mut Context<'_, S>,
    node: &Node,
    from: u64,
    to: u64,
) {
    if let Some(block) = node.borrow().content.block()
        && from < block.length
    {
        let zeros = vec![0; (to.min(block.length) - from) as usize];
        cx.cpu
            .shared_write(block.id, from, &zeros)
            .expect("the block holds the file's first pages");
    }
    changed_bytes(node);
}

/// Writes the pages a backend file's block holds in `[from, to)` back, up to the file's end, if a
/// shared mapping has been able to write them: waiting for the answers, or with nothing to wait
/// sending them on their own. Every page held goes, changed or not, since which were written is not
/// known.
pub(crate) fn backend_write_back<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    node: &Node,
    from: u64,
    to: u64,
    wait: bool,
) -> Result<(), Errno> {
    let (handle, size, block) = match &node.borrow().content {
        Content::Backend(remote) => match &remote.pages {
            Some(pages) if pages.dirtied => (pages.handle.clone(), remote.size.load(), pages.block),
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    let Some(runs) = held_runs(node, from, to.min(size).max(from)) else {
        return Ok(());
    };
    let mut wrote = false;
    for (start, stop, held) in runs {
        if held {
            let bytes = block_bytes(cx, block, start, stop);
            crate::fs::backend::write_back(&handle, start, &bytes, wait)?;
            wrote = true;
        }
    }
    if wrote {
        kernel
            .counters
            .count(cx.host, crate::reason::BACKEND_PAGES_WRITTEN_WHOLE, 0);
    }
    Ok(())
}

/// Makes a file mapping show its file's block, except at a private mapping's own copies.
pub(crate) fn show<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
pub(crate) fn show_all<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
fn map_shown<S: crate::System>(
    cx: &mut Context<'_, S>,
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
fn grow<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
fn moved<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    node: &Node,
    old: Option<Block>,
    new: Block,
) -> Result<(), Errno> {
    match old {
        Some(old) => {
            let away = kernel.live_maps_away();
            let here = kernel
                .processes
                .values()
                .map(|process| (process.memory, &process.map));
            for (memory, map) in here.chain(away.iter().map(|(memory, map)| (*memory, map))) {
                for mapping in map.borrow().iter() {
                    if let Some(offset) = mapped_offset(&mapping.backing, node) {
                        map_shown(cx, new, memory, mapping, offset)?;
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
    let writable = |map: &crate::process::Map| {
        map.borrow().iter().any(|mapping| {
            mapping.allowed.contains(Permissions::WRITE)
                && mapping.shared
                && mapped_offset(&mapping.backing, node).is_some()
        })
    };
    kernel
        .processes
        .values()
        .any(|process| writable(&process.map))
        || kernel.live_maps_away().iter().any(|(_, map)| writable(map))
}

/// The file offset a mapping starts at, if it maps `node`.
fn mapped_offset(backing: &Backing, node: &Node) -> Option<u64> {
    match backing {
        Backing::File {
            node: mapped,
            offset,
            ..
        } if Arc::ptr_eq(mapped, node) => Some(*offset),
        _ => None,
    }
}

/// After a file's bytes or size changed: the pages of its mappings wholly past its end fault with
/// `SIGBUS`, those before it are reachable again, and code run from it is stale.
pub(crate) fn changed<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    node: &Node,
) -> Result<(), Errno> {
    let size = match &node.borrow().content {
        Content::Regular(regular) if regular.block.is_some() => regular.size(),
        Content::Backend(remote) if remote.pages.is_some() => remote.size.load(),
        _ => return Ok(()),
    };
    let away = kernel.live_maps_away();
    let here = kernel
        .processes
        .values()
        .map(|process| (process.memory, &process.map));
    for (memory, map) in here.chain(away.iter().map(|(memory, map)| (*memory, map))) {
        map.borrow_mut()
            .set_file_end(cx.cpu, memory, node, page_up(size))?;
        for mapping in map.borrow().iter() {
            if mapping.permissions.contains(Permissions::EXECUTE)
                && mapped_offset(&mapping.backing, node).is_some()
            {
                cx.cpu
                    .code_changed(memory, mapping.start, mapping.end - mapping.start, &[]);
            }
        }
    }
    Ok(())
}

/// Ends the blocks of files no mapping shows any more, here or in a context away, moving their bytes back to the files: a
/// backend file's are written back, with nothing waiting, if a shared mapping may have written them.
pub(crate) fn release<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>) {
    let processes = &kernel.processes;
    let away = kernel.live_maps_away();
    let mut unmapped = Vec::new();
    kernel.files.shared.retain(|node| {
        let shows = |map: &crate::process::Map| {
            map.borrow()
                .iter()
                .any(|mapping| mapped_offset(&mapping.backing, node).is_some())
        };
        let mapped = processes.values().any(|process| shows(&process.map))
            || away.iter().any(|(_, map)| shows(map));
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
