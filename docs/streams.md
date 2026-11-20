# Streams: standard input, output, error and the terminal

The plan of the streams track. The owner's rulings are in the brief (orchestrator/brief-streams.md);
where this plan and the brief disagree, the brief wins.

## The interface

A byte stream leaves a kernel through an end: a small handle, local to one kernel, whose other side
the embedder holds. Names below marked (suggested) are for the owner to rule on.

- `wcontainers_abi::End(u32)` (suggested): the handle. The embedder names the ends made at start;
  the kernel will name the ones it makes later (connect, accept, import), from numbers no live end
  has. An end made at start is the same kind of thing those will be.
- On `Host`, kernel to embedder:
  - `send_stream(end, bytes)`: data, never more than the credit the embedder has given.
  - `credit_stream(end, bytes: u64)`: the embedder may send this many more bytes (added, not a
    total).
  - `close_stream(end)`: the kernel sends nothing more on this end, data or credit.
- Events, embedder to kernel:
  - `Event::StreamData { end, bytes }` (suggested): data, never more than the kernel's credit.
  - `Event::StreamCredit { end, bytes: u64 }` (suggested): the kernel may send this many more.
  - `Event::StreamClosed { end }` (suggested): the embedder has let go of the end; it sends nothing
    more and takes nothing more.
  - `Event::TerminalResized { end, columns, rows }`: a terminal end's new size (the brief: a
    terminal is an end plus its size).
- What is left of the old surface: nothing. `Host::output`, `wcontainers_abi::Stream` and
  `Event::TerminalInput` are gone; every byte in or out of descriptors 0 to 2 goes through ends.

At start, `Start::standard` (suggested) says what descriptors 0 to 2 are:
- `Standard::Pipes { input: Option<End>, output: End, error: End }` (as `docker run` and
  `docker run -i`): 0 is `/dev/null` without input, else the reading side of a pipe whose writer is
  the input end; 1 and 2 are the writing sides of two pipes whose readers are the output and error
  ends.
- `Standard::Terminal { end, columns, rows }` (as `docker run -t` and `-it`): a pseudo-terminal
  pair made as runc makes it (`/dev/ptmx`, the slave opened as `TIOCGPTPEER` opens it, made the
  first process's controlling terminal, at that size); 0 to 2 are the slave; the master's other side
  is the end. Without `-i` the embedder simply sends nothing.

### Credit, embedder to kernel (input)

- The kernel gives credit only for space it has: for a pipe, the free pages and the space left in a
  last page the embedder's bytes may join, less what it gave that has not come; for a terminal,
  the space in the pty's buffer toward the slave (`pty_write_room`: `PTY_BUFFER_LIMIT` less what it
  holds), less the same. It gives at start and again when that space has grown by a page or more,
  or when nothing is outstanding and any space is free.
- Data that comes is put in at once: into the pipe as one write packed into pages (readers woken,
  their I/O signals sent), or into the master as a master write (the slave's line discipline takes
  it: echo, canonical editing, the signal keys). On a terminal the space was promised, so it fits.
- The pipe's space is not held for the embedder: a guest writer to that same pipe (by opening
  `/proc/<pid>/fd/0` for writing) uses it as on Linux, and `F_SETPIPE_SZ` refuses only a size below
  what the pipe holds. The embedder's bytes that then find no space wait in the pipe's inbox, at most
  the credit given, and go in as readers make space, as the bytes a writer outside Linux has not yet
  written wait in that writer. The embedder closing its end lets go of the pipe's writer once the
  inbox is in.
- A guest read when nothing has come waits, or gets EAGAIN, as on a pipe or terminal with a slow
  writer; poll, select and epoll see the data when it comes.
- Data past the kernel's credit (an embedder that breaks the rule): the part past it is dropped and
  counted (`stream data past its credit`, with a warning). Nothing is buffered beyond the credit.

### Credit, kernel to embedder (output)

- The kernel sends only within the credit the embedder gave. A guest write to a pipe whose reader is
  an end goes out at once as far as credit allows when the pipe holds nothing; the rest stays in the
  pipe, which is Linux's own 64 KiB, and once it is full the writer waits or gets EAGAIN, as a
  writer whose reader is slow. Credit coming sends from the pipe and wakes writers.
- On a terminal the slave's output goes through its line discipline into the master's buffers
  (Linux's), and the kernel sends from them within credit; full, the slave's writer waits or gets
  EAGAIN, and echo is held as Linux holds it.
- So the kernel never holds more than Linux's own buffers for these files.

### Closing

- The embedder closes an input end: the kernel lets go of the pipe's writer once what was sent is
  in: readers get it, then end of file (read 0, POLLHUP).
- The embedder closes an output end: the kernel lets go of the pipe's reader: a later write gets
  SIGPIPE and EPIPE, as on a pipe whose reader closed; what the pipe held is dropped.
- The embedder closes a terminal end: the master is closed: the slave is hung up as Linux's pty
  hangs it up (SIGHUP and SIGCONT to the session leader, reads 0, writes EIO).
- The kernel closes an output end when no writer is left and all it held is sent; a terminal end
  when the slave has no opener and all the master held is sent (a master read would give EIO);
  an input end when no reader is left (data still coming is dropped, as the shim's write gets
  EPIPE).
- After its own close the kernel sends nothing on the end. The embedder frees an end by closing it;
  if the embedder closes first, the end is gone and the kernel says nothing more about it.
- `Next::Finished` can come while output is still held for want of credit; the embedder gives
  credit until each output end is closed.

## Linux behaviour to match, and the Docker command that shows it

On this Mac's Docker (ARM64 Linux), `alpine:3.22`:
- Without `-i`: 0 is `/dev/null` (fdinfo flags 0400002), 1 and 2 are pipes (flags 01):
  `docker run --rm alpine:3.22 sh -c 'ls -l /proc/self/fd; cat /proc/self/fdinfo/0'`.
- With `-i`: 0 is a pipe (flags 00, mode `fifo 600`): `echo hi | docker run --rm -i alpine:3.22
  sh -c 'ls -l /proc/self/fd; cat /proc/self/fdinfo/0'`.
- With `-t`: 0 to 2 are one open file of `/dev/pts/0` (fdinfo flags 02000002), devpts mounted with
  `gid=5,mode=620,ptmxmode=666`, the slave also mounted at `/dev/console`, the first process its
  session's leader with it as controlling terminal (`/proc/self/stat`: tty_nr 34816, tpgid 1),
  stty settings `icrnl ixon opost onlcr isig icanon iexten echo echoe echok echoctl echoke`, size
  0 0 when the client's output is not a terminal: `docker run --rm -t alpine:3.22 sh -c 'ls -l
  /proc/self/fd; stty -a; cat /proc/self/stat'`.
- Input cases (`docker run -i`, input from the test): a pipeline reading all of it, end of file,
  input larger than every buffer to a slow reader, a reader that closes 0 early while input still
  comes, input arriving while blocked in read, poll, select and epoll, a child inheriting 0 to 2,
  redirecting them away. Each is a guest program or shell line run by the test with the same input
  in both.
- Output cases: large output to a slow embedder (the guest blocks, or gets EAGAIN with O_NONBLOCK,
  and everything arrives in order); output after the embedder closed its end gives what a closed pipe
  gives. Docker's daemon reads a container's output for its log even when no client reads, so a
  closed reader is shown inside the reference: the same program run on a pipe whose reader it
  closed itself (`docker run --rm alpine:3.22 sh -c 'trap "" PIPE; (sleep 1; echo x; echo $? >&2)
  | true'` prints 1).
- Terminal cases (`docker run -t` and `-it`, the test holding a pty for the docker client):
  isatty, the size and a resize with SIGWINCH, canonical and raw input, echo, ^C and ^Z reaching
  the foreground process group, ^D at the start of a line.

## How it is built

- abi: `End`, the three `Host` calls, the four events; `Stream`, `Host::output` and
  `Event::TerminalInput` removed.
- kernel: a table of ends, each holding the kernel's side as an ordinary open file (the pipe's
  writer or reader, the pty's master), its send credit and whether the kernel closed it. Pipes and
  ptys need nothing special for it (the pipe's old "host reads it" field goes); a pipe knows the end
  that reads it and keeps the credit it gave and the inbox, a pty the space it promised. The ends are served when the kernel settles, once an event for an
  end or a wake on one of their pipes or terminals says something may have moved; a write to a pipe
  an end reads goes out at once when the pipe is empty and credit allows.
- test host: the ends' other sides, with input fed in seeded pieces as credit allows (when the
  container waits, or after many turns of one that never does, as a real embedder's bytes come
  slower than a program reaches its next wait), output credit given all at once or slowly (or
  never), a scripted conversation (wait for some output, then send, resize or
  close) that the Docker reference follows too, and a check that the kernel never sends past
  credit. A rule-breaking mode sends all its credit and a fixed excess each time.
- A program compared with Docker must not show where its reads end (how much one read returned),
  only what it read: the seed cuts our input into its own pieces and Docker's client cuts the
  reference's into others, so both are right and differ. A reader that takes a stdio buffer and
  leaves the rest (busybox `head -c`) is such a program; `dd bs=1` and the shell's `read` are not.
