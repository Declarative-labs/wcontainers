# Status of the parallel-threads plan

2026-11-19. The plan is docs/threads.md. Nothing is built. The plan was written, checked by an independent
agent six times, and corrected after each check (the six lists are below, newest first). The sixth
correction has had no check of its own, and the plan says so at its top, under "Known faults". It
stops here until the owner has ruled on its section 16.

## The sixth check's findings, and how each was settled

Two blocking and six smaller. None is argued against: all eight were right. Each was checked against
the code, and against docs/native.md where it names it, before it was settled. Four small experiments
were run for this correction (items 19 to 22 under "What was measured"), and the sixth checker's own
model is recorded as item 18.

1. **Two call-ins could wait on each other for ever** (blocking). Right. The checker's model was read
   and run again; it reaches the stuck state in four steps. The code was read for the cure's premise:
   a woken call runs again from its start, from the thread's saved registers (kernel/task.rs:1-6,
   :343-375). Settled in 4.3 and 4.5:
   - a mark keeps a group's threads out of guest code and nothing else. A thread whose call was woken
     for a call-in (a "waiter") is put forward whatever is called in, its own group included;
   - when its call has run it goes back to guest code only if its own group is not called in by
     another call; otherwise it stays ready with its call answered. This is the part the check said
     its control did not settle;
   - a waiter of a `Serial` group is still chosen only when none of its group's threads runs, so the
     lock rule of 7.2 is untouched; the sibling in its way is asked in at once (section 5);
   - a mark belongs to its call and names a memory. It comes off when the call has run or its thread
     is ended.
   4.5 no longer says "nothing can wait in a circle" for a reason that 4.3 contradicts. It gives the
   three things a waiter needs, none of which is another mark coming off. The rules were modelled and
   searched (item 19): the checker's case and a larger one, on one to four processors. Section 12
   has the test, "two call-ins that want each other's groups", with a count in the test build,
   "call-in with nothing to move it".
   - Found with it: a call must name every group it needs at once, its own included when it joins.
     Named one at a time, a woken call could have to park again for the next, with its first mark
     still on. 4.5 says so, and the model checks that a woken call never parks again for its
     call-in.
   - Found with it: section 5's condition counted a thread with a slice as "due in" for a waiter
     too. A slice can be milliseconds, and a whole group stands still meanwhile. A waiter's way is
     now cleared at once.
2. **A process given `None` changed the shared scheduler with entries that were never logged**
   (blocking). Right, and the claim "a replay cannot get stuck" was false with such a process
   present. The code was read: every return charges the thread and moves the queue's least weighted
   time (`account`, kernel/task.rs:77-86), which woken threads are placed against (:555-560), and
   the tick and the slice read the queue's length (:43-53, :88-104). Settled in section 3 and 4.6
   as one rule: **a kernel is recorded whole, or not at all.**
   - A system says at the start whether it orders entries. If not, nothing is asked or logged,
     whatever the configs say: that is where the ruling's "with `None` nothing is recorded" holds.
   - If it does, every entry of every thread is asked about and logged, and no process runs below
     `External`. A process given `None` there runs in force `External`, counted and warned.
   - What the kernel takes from the system inside an entry (the clock it charges by, ids, random
     bytes) is part of the record, as the host's answers are today (docs/runtime.md section 12).
   - The check's own alternative was to log choices and charges for `None` threads. The plan logs all
     of an entry or none: with choices and charges only, what a `None` process writes to a pipe or
     a file would go unchecked, and an `Exact` reader could differ with nobody told.
   - This changes what the owner ruled for one case, so it is put to him as the second question of
     decision 1, with three other answers and what each costs.
   - T1's bar no longer runs a `None` process unlogged beside an `Exact` one. Until T8 builds
     `External`, a recorded kernel accepts only `Exact` (section 13). The test moved to T8, with the
     check's case as one of its runs (section 12, "a kernel is recorded whole").
   - Found with it: **a replay that has left its recording must end, not wait.** With `External` a
     process may make other entries than it did. The earlier form answered `Later` to a choice of
     the wrong thread, and `next` then made the same choice for ever. 4.6 now names four ways the
     replaying system sees that the replay has left its recording, each of which ends it, and the
     one case no rule of the kernel's can end (a thread that computes on and never makes its due
     entry). Section 12 has a test from changed logs.
   - Found with it: (`Parallel`, `External`) runs `Serial` until it is built, so in a recorded kernel
     threads run at the same time only as (`Parallel`, `Exact`). Sections 2 and 3 say so.
3. **`suspend` would take `Rc` clones out from under the kernel's lock.** Right: `FileRef`
   (kernel/file.rs:94) and `Map`, `Handlers`, `Files`, `Fs` (kernel/process.rs:243, :247, :251, :262)
   are shared across fork and the `CLONE_` flags, and every descriptor lookup hands out a clone
   (kernel/file.rs:181, kernel/syscall/io.rs:33). Nothing is wrong in T1, which has no `suspend`.
   Settled in 4.2 as a rule asked of the interface track: what `suspend` hands out shares no `Rc`
   with what stays; a shared object's reference is kept in the kernel under a key; a dropped value
   lets go under the lock, at the kernel's next entry. Of the check's three suggestions this is the
   third ("suspend detaches what it shares"). The first (`!Send`, dropped only under the lock) was
   rejected because a `resume` from another thread is what the value is for; the second (`Arc`
   first) because every descriptor call would pay for it.
4. **A `System` method called from a guest hook could enter the kernel and hang a group stop.** Right
   (rt/lib.rs:291-299: the clock's answer calls `ran` and `monotonic_nanoseconds`, `warn` calls
   `cpu_warning`; the clock hook is wt/lib.rs:2124-2131). Settled in 4.2 with both of the check's
   suggestions: the three methods' doc comments say they may not call the kernel or wait for anyone
   who does, and a mark of the worker's makes such a call a panic that names it. The mark is looked
   at where the kernel is entered from outside the loop, not at a system call. That choice was
   measured (item 21): the test at every take of the lock moved a round by up to 1 ns on the
   laptop. What cannot be checked is said: a system that waits in `ran` for a lock of its own that
   a caller of the kernel holds.
5. **The interface was not mapped onto the native executor.** Right. docs/native.md was read at
   ab448e58 and at its head (a7099033), which has the owner's later word that the embedder's
   `System` runs inside the virtual machine (lines 1092-1095). Section 10 now has the map: every
   call the kernel makes at an entry is on the inside half, with no crossing; calls into the kernel
   are made by the system's code inside; a host program asks through the link and gets a copy, as
   native.md's decision 11 recommends; the lock's holder is the virtual CPU's number, with the
   reason that is sound. Sections 5 and 6 and decisions 8 and 9 no longer say "any host thread"
   without saying what that is there. KVM's bound was read on the laptop (item 22):
   `KVM_CAP_MAX_VCPUS` 4096.
6. **The native output copied barriers always, and so charged every `Serial` run.** Right. Settled in
   section 10: the native output has the same two forms by ordering as the WebAssembly output. Its
   Serial form emits nothing for `dmb`, `dsb` and `isb`, as native.md has it today, and leaves
   `mfence` and lock prefixes out on x86-64; its Parallel form copies them. So `Serial` pays
   nothing, and there is nothing to put to the owner beyond decision 7's sentence. What the
   barriers would have cost was measured as hardware instructions (item 20): 1.2 to 8.2 ns each on
   this Mac, `mfence` 17.2 ns on the laptop.
7. **The executor's lock could not be let go before the kernel's in today's structure.** Right: the
   entry is over inside `Answer::exit` (rt/lib.rs:264-289), called from inside the executor's loop
   (wt/lib.rs:2325), which puts the store back only afterwards (:2336-2344). Settled in 4.2 and 7.2
   by the check's first suggestion: `Carry::Leave` carries the kernel's handle, and the executor
   lets its own lock go and then drops it. Section 12's lock-rule test has a run for it.
8. **The known fault of a pinned thread with no worker was reached by default** (the check's last
   finding). Right: the processors are the container's and the worker count defaulted to the
   host's. Settled in section 5: a live run has one worker for each processor, and the setting
   `RuntimeConfig::host_threads` is gone. Known fault 2 is rewritten: what is left open is running
   fewer workers than processors, which is not designed, and what a container with more processors
   than its host has cores costs, which is not measured. Decision 2 asks the owner to confirm it.

Section 16 still has ten decisions. Decision 1 gained the second question (what `None` means in a
recorded kernel). Decision 2 gained the worker for each processor. Decision 8 says who "any thread" is
on the native executor and what a call from a guest hook gets. Decision 9 says its calls are on the
inside half there, and gained `Carry::Leave` with the handle and the rule for `SuspendedProcess`.
Decision 10 lost `RuntimeConfig::host_threads` and gained "recorded kernel" and "waiter".

Read for this correction: the brief and the interface notes to their ends; the sixth checker's model
and its two logs (wcontainers-scratch/threads/check6); docs/native.md at ab448e58 (sections 3.4, 5.1,
5.3 to 5.6, 6.1 to 6.3 and its decisions) and what its head changed since (`git diff ab448e58
origin/phase-2-native -- docs/native.md`); kernel/task.rs:1-6, :40-106, :340-376, :548-586;
kernel/file.rs:94, :181; kernel/process.rs:238-264; rt/lib.rs:100-170, :190-300; wt/lib.rs:2110-2140,
:2236-2345; docs/runtime.md section 12.

## The fifth check's findings, and how each was settled

Two blocking, six smaller, and one that reports an experiment and no fault. None is argued against:
all eight faults were right. Each was checked against the code before it was settled. Two small
experiments were run for this correction (items 15 and 16 under "What was measured"), and the fifth
checker's own is recorded as item 17. The full text of the findings is in
wcontainers-scratch/orchestrator/threads-plan-open-findings.json.

1. **A ready thread of a `Serial` group could wait for ever behind its own running sibling**
   (blocking). Right, and with one processor too. The code was read for it: a fair slice is given
   only when the run queue is not empty at the hand-out (kernel/task.rs:43-53); the kernel's deadline
   is folded into every slice today (task.rs:55-60), which the plan takes out; and `preempts` says
   no for a batch thread and inside the granularity (kernel/schedule.rs:224-238). Settled in section
   5 as the check suggested, as one condition for every ready thread and not a case for siblings:
   every ready thread that could be chosen has a running thread in its way that is due in. In its
   way: its `Serial` group's running thread, or else the running threads on the processors its mask
   holds. Due in: it has a fair slice, or the kernel has asked it in. When nobody in the way is due
   in, the one with the most weighted time is asked in, and the tick decides. Section 11 aims the
   wake-up comparison at a thread in the woken one's way, as the check also asked, and says that
   the comparison is not what bounds the wait.
   - Found with it: **the same thread could still starve when it is pinned.** A `Serial` group's
     thread that gives way for a sibling pinned to another processor is the only thread of the group
     its own processor may take, so it was handed out again at once. 4.3 now has a `Serial` group
     put forward one thread, its ready thread with the least weighted time. With no thread pinned,
     and with one processor, that chooses as before. Its cost is known fault 4.
   - Found with it: how the condition is kept cheap had to change. The two counts of the fourth
     correction cannot see this half. Section 5 now has one mark, set where the data the condition
     reads is written, and three counts behind it. A system call answered in place tests the mark
     and nothing else. A build for tests ignores both and walks at every lock release.
   - Found with it, and not settled: with fewer workers than processors, nobody asks for the
     processors left over, so a thread pinned to one of them never runs. It is known fault 2.
2. **Two `Serial` groups that merge could leave one `Serial` group with two running threads**
   (blocking). Right. Nothing called anybody in when no config changed, and 4.5 said "nobody" for a
   new mapping in the caller's own `Serial` memory. Settled as the check's first suggestion: a join
   calls in every running thread it binds, and the call that caused it parks before it has any
   effect (section 3, 4.5). The calls that can join groups were looked up: `mmap`
   (kernel/syscall/memory.rs:109), `shmat` (ipc.rs:356), and exec through its program files
   (kernel/loader.rs:901-905, kernel/shared.rs:478-487). `mremap` and `mprotect` work inside one
   memory. 4.4 items 3 and 7 and the lock rules of 7.2 now name the rule they stand on. Section 12
   has the test, and a count in the test build, "two threads of a Serial group running".
   - Found with it: **4.5 contradicted 4.3 on who may run while a group is called in.** 4.5 said the
     mark never keeps a woken call from running. 4.3 runs a woken call only when its thread is
     chosen, and a called-in group's threads are not chosen. A group that calls itself in (a
     (`Parallel`, `Exact`) group does, and so does a join) would have waited for ever. Both now say
     that the thread whose call wanted the group is chosen as any ready thread is.
3. **7.2's rule on `block_write` would have left memories stale.** Right: `block_write` writes into
   every memory that shows the block (wt/lib.rs:1409-1433), and the rule's reason only explained
   not reading a memory. The rule is rewritten by its two callers. The copies made when a `Serial`
   thread leaves reach memories of its own group, whose locks are free. A write of the kernel's
   reaches memories of the caller's group, memories called in first, and `Parallel` memories. No
   memory is left out as a destination. What is left of the old rule is that nobody reads another
   `Serial` group's memory as a source.
4. **A guarantee could become `Exact` through a merge with no starting point and nothing announced.**
   Right, and the plan's own argument in section 6 was the one that showed it. Settled by the
   check's first suggestion: a hook, suggested `on_execution_config_in_force_changing`, fired before
   the change when the process is whole and stopped, which finding 2's cure provides. Decision 1
   says what an embedder that does nothing in it gets. The test in section 12 is now one a T1 test
   system can pass: it saves the container at a stop it asks for in the hook, and replays from the
   save. The test also needed a way to give two processes different configs before `resume` exists;
   it says a build for tests does.
5. **`kernel.process(id)` from inside a hook would wait for ever.** Right: `lock_api::RawMutex` has
   no way to ask who holds it. Settled by the check's first suggestion, kept as small as it can be:
   the kernel writes its holder's name in a word (`lock_api::GetThreadId`) and reads it only when
   it finds the lock held. The check's concern was wider than the one call, and the plan says so:
   any entry point called from inside a hook, a `Host` call or `arrived` is in the same place, and
   panics naming the call. Measured (item 15): the word cannot be told from the noise.
6. **`Serial` threaded programs were measured nowhere, though T2 changes how they run.** Right.
   Section 12's "`Serial` must not get slower" now has four measurements of threaded programs run
   `Serial`, in T2's bar, and 7.2 says what is done if they fail: the Serial form keeps one store
   for each process.
7. **Aligned plain accesses seen whole are a fourth thing relied on and not promised.** Right, and
   the premise it asked to have confirmed was read: ECMAScript's `IsNoTearConfiguration` (25.1.3.13)
   answers false for an unordered 64 bit integer access, and the WebAssembly threads proposal's
   overview says nothing of it. 7.6 has it as the fourth, section 12 has its test, and it was run
   now in both compilers on both hosts (item 16): no mix in 1,000,000,000 loads each.
8. **`next` did work for a thread before it knew it would hand it out.** Right
   (kernel/task.rs:343-375). Settled as suggested: `next` makes one entry for each thread it
   chooses, asked about when the thread is chosen and before its woken call runs, and the entry
   counts whether or not a thread is handed out in the end (4.3, 4.6). Section 12 has a test.
9. **The host-supplied shared memory works** (the checker's experiment, no fault). Recorded as item
   17; 7.2 and section 15 no longer say "read, not run".

**The brief's last section, added for this round.**
- *`&self` on `System`, and `System: Sync`.* Section 5 says what it means here: the plan asks for
  `&self` already and needs it from T1, because several workers call `turn`, `ran` and `idle` at
  once; `Sync` is asked by the compiler of any system given to a second worker, and the plan
  recommends that the trait says so from T1. In decision 9.
- *State kept per process.* Section 5: it fits the plan's locking as it is, takes off an embedder
  the locks around what it keeps about one process, takes off nothing else, and would ask one bound
  (`Send`). The plan builds on neither proposal.
- *`Process::replace_execution_config`.* Taken out of the plan's interface. The interface track
  measured the return of `run` it was meant to save as nothing at a program start
  (orchestrator/interface-measurements.json), and the plan was read again for another need of it
  and has none (section 6).
- *Known faults at the top.* Six, of which the first is that this correction was not checked.

Section 16 still has ten decisions. Decision 1 gained the hook, decision 7 the holder's word,
decision 8 what a call into the kernel from inside a hook gets, and decision 9 lost
`Process::replace_execution_config` and gained the hook, `System: Sync` and the two proposals.

## The fourth check's findings, and how each was settled

Two blocking and nine smaller. All eleven were right, and none is argued against. Each was checked
against the code before it was settled. Settling them turned up four more faults of the same kinds,
which the check did not have; they are marked "found with it". Nothing was built and nothing new was
measured for this correction: the fourth checker's one experiment is recorded as item 14 below, and
its files were read again.

1. **An idle processor could stay idle with a thread it could run** (blocking). Right. The plan woke a
   processor when an entry made a thread ready, and three changes make a ready thread runnable with
   no thread becoming ready: a call-in's mark taken off, a `Serial` sibling leaving its processor, a
   mask gaining a processor. A fourth of the same kind is a config in force that becomes `Parallel`.
   Settled in section 5 as the check's second suggestion, a condition and not a longer list: when
   the kernel's lock is let go, no processor is marked idle that `next` would hand a thread to. It is
   kept cheap by two counts (idle processors; ready threads their groups would let run), so a system
   call on a busy container tests two numbers. A test build makes the walk at every release and
   counts a breach. 4.5 says why taking the mark off wakes processors, 4.3 covers the widened mask,
   and section 12 has the check's two cases and two more.
   - Found with it: **a ready thread could wait for ever behind threads that have no slice.** A
     thread gets a fair slice only if somebody waits at its hand-out (kernel/task.rs:43-53). With
     several processors a thread can become ready through another processor's entry while every
     processor runs a thread with no slice, and nothing then brings one in. Section 5 gives the
     condition a second half: one of them is asked in, and the scheduler's tick decides
     (kernel/task.rs:88-104).
2. **A signal sent to a process could be stranded** (blocking). Right: `complete` picks one thread
   and tells nobody else (kernel/signal.rs:459-492), and nothing in the kernel picks again (`grep
   -rni retarget crates/wcontainers-kernel/src` finds nothing). Settled in 4.5: the kernel picks
   again when a thread's mask gains a signal pending on its process and when a thread ends with such
   a signal not blocked, as Linux does. Linux's source was read for it (6.12, kernel/signal.c:
   `retarget_shared_pending` at 2974-2998, called from `exit_signals` at 3033 and from
   `__set_task_blocked` at 3076). The mask is written in nine places today; they become one
   function.
   - Found with it: the check places this in `Parallel` groups. **It reaches `Serial` groups too**,
     so it is T1's work and not T2's. The picked thread runs on another processor, blocks the
     signal in its next call, and its sibling parked in `read` is never woken. Section 12 tests
     both.
3. **The argument that the executor's try-lock never fires had a gap.** Right: the asked-in bit can
   be set after the thread has looked at it and before it has the kernel's lock. Settled in 4.5 and
   7.2 as suggested, and one step further. The bit is taken off inside the entry, after the lock is
   taken. And the lock rule no longer looks at the bit: a worker lets its process's lock go before
   the kernel's lock whenever its thread is not told to carry on.
   - Found with it: **the kernel acted on threads that were not running at the end of every
     entry.** Today `settle` runs a woken thread's call again, and writes a signal's frame on a
     waiting thread's stack, in whichever entry comes next (kernel/task.rs:343-404). With several
     processors that touches a `Serial` group's memory while a sibling runs and has its lock. 4.3
     now says both are done when the thread is chosen for a processor. 7.2 also says what happens
     when the call that wanted a group runs again on the very worker of the thread that came in
     last: that worker has the lock and uses it.
4. **The owner's guarantee for hooks was narrowed without asking him.** Right. The narrowing stays,
   and is now stated where it is made (4.4 item 7, section 6) and asked (decision 8), with what
   keeping the guarantee whole costs: the kernel would call the group in before each hook that
   fires with other threads running, so every watched access of a (`Parallel`, `Exact`) process
   would stop every thread of the group through the kernel.
5. **Reading a process while the container runs was closed off.** Right, and for no reason.
   `kernel.process(id)` now answers at any time, under the kernel's lock; reads of the kernel's
   records are always legal and are not kernel entries. One rule says what else a handle may do: an
   operation is carried out with whatever stop it needs when that stop can be made in place, and
   answers an error only where it would need a call-in (section 5, decision 8).
6. **A permission lost on a page of another thread's live frame was caught only for watches.**
   Right (tr/contract.rs:20-23). Settled in 4.5: after a page that allowed reading and writing has
   lost a permission, been unmapped or remapped under a group stop, each other thread of the memory
   gives up its frames, at 300.8 ns each (docs/runtime.md section 10).
   - Found with it: **the plan's test for which threads, taken from the x86 plan, was not sound.**
     It asked whether the page lay at or above the thread's stack pointer in the same mapping. A
     frame is kept across a call whenever the callee gives the stack pointer back unchanged
     (tr/full.rs:2835-2853), so a thread that moves to another stack inside a call still relies on
     its frames on the first when it comes back. Go's `systemstack` has that shape. The test is now
     by the page: a frame is only proved over pages that allow reading and writing
     (tr/full.rs:730-731), so only such a page makes anybody give up frames. Section 8 uses the
     same test for watches, which costs every hand-over from a writer one more 300 ns. The x86
     track is asked to use it for code on a live stack.
   - The plan also said such a thread "leaves `Execution::run`". It does not: it leaves the
     translated code and is entered again from the bottom loop, inside `Execution::run` and with no
     kernel entry, as a thread whose code changed under it does today (wt/lib.rs:1729-1733,
     :2314-2317). The plan now has a word for it, "gives up its frames", and uses it for code drops
     too.
7. **A 128 bit atomic of a process with one thread could lose a write of the kernel's.** Right.
   Settled in 7.3 by the check's second suggestion: a write from outside into a memory whose byte
   is 0 is made under a group stop of that memory, so its one thread is at a stop point and not
   inside the inline form. Rejected, with the reason: the byte at 1 for every `Parallel` process,
   which would take from a process with one thread what the byte is there to spare.
8. **Nothing said how a timer expires when every processor is busy.** Right
   (kernel/task.rs:55-60). Settled in section 5: the kernel's deadline is in nobody's slice. The
   kernel announces its earliest deadline when an entry moves it (a hook, suggested
   `on_deadline_changed`), the system hands in one `Event::Timer` for the container, and that
   entry stops nobody by itself. An expiry with every processor busy brings in one thread at most
   (item 1's second half). Rejected: the deadline in one processor's slice.
9. **A change T1 needs was scheduled for T2.** Right: `code_line_changed` walks every memory that
   shows the block (wt/lib.rs:2008-2029). "A drop reaches the caller's group only" is now in T1's
   list, and 7.5 says why.
10. **The compiler's pass does four things, not three.** Right: its header lists dead-store
    elimination (cranelift-codegen 0.137.1, src/alias_analysis.rs:57-66) and the optimiser acts on
    it (src/egraph/mod.rs:570-573). 7.6 says four, says why the fourth does not happen to guest
    stores today, and section 12 has the shape in the ordering tests and in the machine-code test,
    so a release that starts doing it is noticed.
11. **The repeatable one-worker mode could not be built on `run` as defined.** Right. Settled in
    4.6 and section 11 as suggested: the test system answers `Later` for every hand-out but the one
    its seed draws, which is a log made up as the run goes. A processor told `Later` is not marked
    idle. The mode holds for processes in force `Exact`, and the plan says so.

Section 16 still has ten decisions. Decision 8 changed (what an embedder may do to a process while
its threads run, in three parts); decision 9's list gained `on_deadline_changed` and
`kernel.process(id)` at any time.

## The third check's findings, and how each was settled

Three blocking and four smaller. All seven were right in what they found. One is corrected in a detail
(finding 3: one of its two examples is not forbidden), and settling it turned up a second fault of the
same kind that the check did not have. Each was checked against the code or by experiment before it was
settled. The full text of the findings is in
wcontainers-scratch/orchestrator/threads-plan-open-findings.json.

1. **The plan was built on rulings the owner had replaced** (blocking). The brief and the interface
   notes were read to their ends again; the brief's last three sections and the notes' last two are what
   the plan now follows. Changed throughout:
   - `ThreadOrdering { Serial, Parallel }` and the guarantee as a field of `ExecutionConfig`; no getter
     on the ordering; `ParallelOptimistic` is the pair (`Parallel`, `External`) (section 2, with what
     each of the six pairs means and pays).
   - The rule for a group whose processes were given different configs now covers both fields: the
     strictest of each (section 3, decision 1). It also says what a guarantee covers between groups: a
     `None` process is not recorded, so nothing it does to others is repeated.
   - The count is only in the code of a process in force `Exact`, and is a field of the cache key (7.2).
     T1 and 4.6 no longer assume every `Serial` group replays exactly: a thread in force `None` is not
     asked about at the recording point, and runs free turns.
   - A design for `External` (4.6): entries in order with no position; a digest of what the kernel took
     from the thread for each entry; a replay that is thrown away when a digest differs; what replays
     surely (a process with one thread that takes no signal while it computes) and what may not. The
     `Parallel` half (atomic instructions in one order) is described from the prototype's results and
     recommended against (decision 3). A new milestone, T8, builds the `Serial` half.
   - (`Parallel`, `Exact`): the rule "stops only at a kernel entry of its own" is now for groups in
     force `Serial` or `Exact`, each for its own reason (4.4 item 5). A change of the kernel's in such a
     group is a call-in, never a group stop in place (4.5). Three uses of holding a thread in place
     stay there, all of which only order memory accesses.
   - No setter: section 6 keeps the weighing short, as the brief asked for it, and says it is settled.
     `suspend` in a hook of a (`Parallel`, `Exact`) process answers an error, because it cannot stop
     the others at positions. The in-hook call (`Process::replace_execution_config`) is weighed and
     recommended (decision 9).
   - The process closure is gone as a rule: "a group ends at the kernel's edge" (section 3).
   - The decisions the rulings already made left section 16. It has ten, from twelve.
2. **A permission change could be torn** (blocking). Confirmed in the code: a piece relies on what it
   found out until its thread's next stop point (tr/contract.rs:17-20); a loop with no call and no
   system call keeps its known pages (tr/full.rs:1130-1146); a guest hook sets neither flag
   (guest-cpu/effects.rs:137-142); the stop check is the only place that forgets
   (tr/full.rs:2162-2177). Settled in 4.5, with the test in section 12:
   - a thread is held only at a stop point. The mark that says "may be in guest code" stays on in the
     clock hook, the float hook and `warn`, which may neither wait nor touch the memory;
   - the three hooks that must wait or touch the memory (code changed, the 128 bit atomic, x86's
     misaligned locked access) are made stop points: the translator saves every register before them
     and forgets what it knew after them, as at a stop check whose hook ran. This is a change to
     translated code, on those three paths only, asked of the translator track;
   - the load code hook is a stop point by where it is called. That a piece keeps nothing across a call
     or a jump out of itself is the contract's own rule; it was checked for loops (tr/full.rs:1143-1146)
     and is left for the translator track to confirm for checks that cover several accesses;
   - found with it: a guest hook saves only its own registers (tr/full.rs:2553-2569), so the earlier
     plan's "a held thread's registers are saved" was also wrong for those hooks. It now holds, because
     a held thread is at a stop point;
   - section 8's "setting a watch stops one thread" has the same cure.
3. **"Plain accesses are compiled to the host's plain accesses, in program order" was false for
   wasmtime** (blocking). Confirmed by experiment, then taken further (items 10 to 13 below).
   - The check's case (a), a repeated load merged into the first, is right, and x86 forbids the result.
     Shown with two threads: a sequence lock's reader accepted 931,346 torn reads of 1,670,130 on the
     x86 laptop.
   - The check's case (c), a store forwarded to a later load, is **not** forbidden by x86: a thread
     reads its own store from its store buffer, so "store 7 to x, load y, load x gives 7" is always a
     legal outcome there. The plan says so (7.6) and does not treat it as a fault.
   - **A third thing the same pass does, which the check did not have, is forbidden on both
     architectures**: it drops a store of a value the thread has just loaded from the same address,
     also under a branch. Seen in the machine code and with two threads (57,368 of 57,368 rounds on
     this Mac, 199,975 of 199,975 on the laptop). So ARM64 guests in parallel were wrong too, not only
     x86 guests.
   - The check's suggested cure, atomic loads for x86 guests, works (0 failures) but costs a load-heavy
     loop up to 1.48 times, and 1.97 times with the alignment test; and it does nothing for ARM64
     guests on ARM64 hosts.
   - The cure the plan takes: compile the code of parallel groups with that pass off. Measured: 0
     failures of either kind on the x86 laptop, 0 dropped stores on this Mac, and 0.0% to 1.8% more
     instructions on the benchmark set. Cranelift's setting for it exists but nothing reads it in the
     pinned version (measured and read in the source), so it needs a small fix sent to wasmtime's
     project and carried as a patch meanwhile (decision 4).
   - That cost would fall on `Serial` runs too if there were one form of code. So the plan now has two
     forms, by the ordering a process was given (7.2). The Serial form is today's code with a shared
     import and pays nothing; the byte, the tested fence and the compiler setting are the Parallel
     form's. This reverses the earlier "one translation for both orderings", with the three choices and
     their measured costs side by side as the brief asks. With no setter, a process never changes form
     while it is in a kernel, which was the main cost of two forms before.
   - T6's ordering tests gained the three shapes the standard four do not have (section 12).
   - The two smaller ARM64 cases: `isb` becomes the tested fence in the Parallel form (7.3); an address
     dependency that cannot change the address is a written difference (7.3, decision 6).
4. **Decision 12 left out its main cost** (not blocking). Right: under the earlier recommendation every
   page that arrives for a waiting thread would stop the whole container. The cost is now stated, the
   narrow alternative is weighed, and the recommendation changed: one call that brings one process's
   group to a stop and announces a hook (section 5, decision 8). The narrow call is rejected because a
   `Serial` group's running thread has its process's executor records locked, so it must be stopped
   anyway.
5. **The plan broke its own lock order** (not blocking). The exception is stated in 4.5 and its rules
   are in 7.2: a `Serial` process's lock is reached from outside only by a try-lock that panics.
   Found with it: the earlier text let a `Cpu` call wait for a thread that was on its way out of
   `Execution::run`. With a try-lock that would have panicked, so a thread that was asked in now lets
   its process's lock go before it makes the entry.
6. **`raise_stop` writes the whole stop word** (not blocking). Confirmed (tr/code.rs:693-699). It becomes
   an atomic or of the thread's own bit (4.5), with a test (section 12). The claim that translated code
   never writes the word is gone.
7. **"The architecture's broadcast invalidation, as Linux does" had no source** (not blocking). Right.
   Section 10 now says each held virtual CPU drops its own cached translations before it is let go, on
   both architectures, with Intel's manual named for why x86 needs it, and ARM64's broadcast form left
   as something the native track may measure.

## The second check's findings, and how each was settled

This list and the first check's below are kept as they were written. Where the third check changed what
they say (one form of code for both orderings, the byte in every run, a thread held inside any hook,
the three-value ordering), the list above holds.

All ten were right, and each was checked against the code before it was settled. None is argued
against. Nothing new was measured for this run: the second checker's two experiments are recorded below
as items 8 and 9, and its sources and logs are in wcontainers-scratch/threads/check2.

1. **A request to stop a thread could be lost** (blocking). Confirmed in the code: the WebAssembly
   executor stores the whole stop word each time round its loop and in the stop hook (wt/lib.rs:2283,
   :1786), and only `StopFlag`'s latch puts a request back (rt/lib.rs:64-74). Settled in 4.5 and section
   5. The stop word becomes a set of bits, each written only by its owner with an atomic or and an
   atomic and. The asked-in bit is taken off only by the thread itself, just before a kernel entry of its
   own, so a request holds until that entry. The held bit of a group stop is a different bit, so neither
   uses the other up. Translated code does not change: it tests the word against zero. The promise is
   part of the stopping handle's interface for every executor, and section 10 says how the native
   executor keeps it. Section 12 has a test that pauses the worker at each place the old loop wrote over
   the word. Cost on a system call: a load and a branch.
2. **A group stop counted threads whose hooks were still touching memory or installing code**
   (blocking). Settled in 4.5, 7.3 and 7.5.
   (a) One stop lock per memory, taken exclusively before any held bit is set; every hook's and every
   `Cpu` call's access to the memory's bytes, table, slots or code lists has it shared; and "held" is
   defined exactly, with a mark per thread. Two 128 bit atomics then go one after the other, and no
   write of the kernel's lands inside one.
   (b) Each code range keeps a count of its drops. A translation reads the count with its bytes and
   installs only if the count has not moved; otherwise it is thrown away and made again.
   Different from the suggested fix in one point: the lock is per memory, not per group. Every change a
   group stop guards is to one memory's own table, code or bytes, and on the WebAssembly executor a
   group in force `Parallel` shows every writable shared page in one place.
   Found while settling it, three more of the same family:
   - the kernel itself makes read, compute, write sequences on guest words that guest code changes too:
     `FUTEX_WAKE_OP`, the owner word of a priority-inheritance lock, the owner-died mark
     (kernel/syscall/futex.rs:682-711, :760-786, :1020, :1163). In a `Parallel` group another thread's
     change in between would be lost. They become a compare and exchange asked of the executor (4.5);
   - `ic ivau` today drops the same line in every memory that shows the block (wt/lib.rs:2008-2010),
     which would change another group's code from a hook. A drop now reaches the caller's group only
     (7.5), with the reason why nothing more is needed;
   - `block_write` goes through every process's records to find who shows a block
     (wt/lib.rs:1413-1414). Each block keeps its own list instead (7.2).
3. **The plan understated what T1 adds to a `Serial` system call.** Settled in 4.2, 4.6, 7.2, section
   12, T1 and decision 7. The lock's cost is now given with both measurements and as depending on the
   work around it. Each other cost is designed away and listed with how: the system is asked about
   entries only if it said at the start that it orders them (the shipped one does not); a `Serial`
   thread takes its process's executor lock once for its whole stay in `Execution::run`, not in every
   hook; the shipped system's per-call state is already atomic (rt/system.rs:256-275); the stop word
   costs a load and a branch. T1's bar is now three builds, one of them with a lock that does nothing,
   so "nothing but the lock was added" is checked and the lock's real cost is measured in place.
   Found while settling it: 4.5 let a page of a `Serial` group gain a permission from another group's
   entry with nobody called in (a file that grows). That broke 4.4 item 5 and the argument for the
   executor's lock above. It is now a call-in.
4. **Names.** `ExecutionConfig`, `RuntimeConfig`, `Process::execution_config()`, `run`,
   `run_until_finished` and `StopReason` are used throughout, and "engine" is gone (the plan says
   "WebAssembly compiler"). This was made as its own commit before the other corrections. The
   executor's own call is always written `Execution::run`, and decision 11 points out that it now shares
   a word with the container's `run`.
5. **`suspend` could not stop a process wherever it may be called.** Settled in sections 5 and 6 and
   decisions 1 and 12. `suspend` never calls threads in. On a handle from `kernel.process(id)` no
   worker is inside `run`; in a hook of a `Serial` group nobody else runs (new item 7 of 4.4, which
   follows from item 5); in a hook of a `Parallel` process it uses the executor's group stop, with
   registers saved in place and a thread that had not yet handed its exit to the kernel put back to the
   instruction that made it. Where `kernel.process(id)` may be called is now a question for the owner
   (decision 12).
6. **Per-thread watches left out the contract's exception for stack frames.** Settled in section 8 and
   T4. A watch that takes a permission from a page at or above a thread's stack pointer makes that
   thread give up its WebAssembly frames, by the x86 plan's own rule for code on a live stack. Checked in
   the translator: a piece entered from the bottom loop knows nothing about its frame
   (tr/full.rs:851-864). T4's program now shares objects on stacks.
7. **The copy of a written shared page must go one way.** Settled in 7.4: in a `Parallel` group it is
   its own copy from the memory to the block's bytes, never `block_write`.
8. **The test clock does not repeat with several workers.** Settled in section 11 and section 12. With
   one worker a run repeats, with any number of processors. With several it repeats only from its
   recording, so stress runs are recorded.
9. **`sched_setaffinity` and one `membarrier` command.** Settled in 4.3, 4.5, section 5 and section 12.
   Each thread keeps a mask; `next` and the idle wake respect it; a thread on a processor its new mask
   leaves out is asked in, and the caller parks until it has come in. `membarrier`'s
   restartable-sequence command asks each running thread in, because a sequence is restarted only in an
   entry of its own thread.
10. **The second checker's wasmtime experiment** is recorded: 7.6, section 14, section 15, and item 9
    below.

## The first check's findings, and how each was settled

All nine were right. None is argued against. Two were settled a little differently from the fix the
check suggested, and two more problems turned up while settling them; both are said below.

1. **Barriers were translated to nothing, and the single translation was unproven for them** (blocking).
   Settled in 7.1, 7.2, 7.3, 7.6, section 9 and section 10. ARM64's `dmb` and `dsb` and x86's `mfence`
   become `atomic.fence`, behind one byte per process that says whether another thread of the group may
   be running. The same byte keeps 128 bit atomics inline in a `Serial` run. Measured: the byte's test
   costs 0.12 to 0.8 ns a barrier, the fence without it 0.7 to 19 ns. A `Serial` run does get slower by
   that test, so it is put to the owner (decision 7), and section 12 has a bar for it.
   Found while settling it: the native output drops barriers too (native.md lines 350-362), and its
   exclusive load has a fix-up that assumes one thread (lines 492-496). Section 10 said "atomics need
   nothing" there, which was wrong for both. Section 10 now asks both of the native track.
2. **Groups that are not joined still reached each other's memory** (blocking). Settled in section 3, 4.4
   item 5, 4.5 and 4.6. (a) Every memory that shows a block which some shared mapping may write is joined,
   private mappings included. (b) Two rules: a thread of a `Serial` group stops only at an entry of its
   own, and a thread ends only in an entry of its own. So the kernel calls a `Serial` group in before it
   touches its bytes, permissions or code from outside. (c) "May write" is what `mprotect` may give
   (`Mapping::allowed`).
   Different from the suggested fix in one point: for a group in force `Parallel` reached from outside, a
   permission lowering or code drop uses the executor's group stop, not `thread_stop` and parking. Such a
   group promises nothing about where its threads stop, and the group stop is cheaper.
   Found while settling it: a fatal signal from outside ended the target on the spot
   (kernel/signal.rs:481-485), which writes into the target's memory under its running thread. It now
   waits for that thread's entry (4.5).
3. **`next` was not a sequenced entry, and a host thread was a processor** (blocking). Settled in 4.3,
   4.6 and section 5. A processor is the kernel's; a worker names the processor it asks for. `next` is an
   entry when it hands a thread out. The kernel no longer hands out numbers: it asks the system whether
   an entry may take effect now, and an entry told "later" never holds a host thread (a returned thread
   is kept aside and its worker freed; a hand-out answers `Idle`). A replay calls `run` in the log's
   order, with one worker or many. 4.6 says why it cannot get stuck.
4. **Decision 8 reopened a ruled question.** Removed. Section 2 states the ruling (Linux's
   `ptrace_may_access` line; `cmdline` is not inspection, `environ`, `maps`, `auxv` and the address fields
   of `stat` are). 4.5 keeps only what this plan adds: `cmdline` is still a read of another group's
   memory, so a `Serial` group is called in for it.
5. **Form B was weighed in replaced words.** Section 6 is rewritten around `suspend`, `resume` and
   `SuspendedProcess`. The two points that assumed a conversion are withdrawn. A group of several
   processes has its own paragraph. The strictest-wins rule is now decision 2, with what
   `Process::thread_ordering()` answers.
6. **The interface list was incomplete and idle could lose a wake.** Section 5 and decision 9. The list
   now has the admission service, `thread_stop`, the hook that wakes an idle processor, the processor and
   thread told to `turn`, `ran`, `arrived` and `idle`, and threads made ready by any entry. The idle mark
   is made under the lock and the wake is kept if it comes early. A thread now always ends in its own
   entry, so none leaves `Execution::run` without its exit.
7. **No bar kept `Serial` from getting slower in T1.** Section 12 has three `Serial` measurements and T1's
   bar names them. Measured for it: an uncontended lock is not visible on this Mac and costs 7 to 9 ns on
   the x86 laptop. So the loop takes the lock once a system call, not twice (4.2), and the rest is put to
   the owner (decision 7).
8. **Two costs of shared-page locks and watches were left out.** (1) 7.4: in a `Parallel` group a written
   shared page is no longer locked again at each turn's end; it is copied when the block's bytes are
   asked for. The group stop at every turn end is gone. (2) Section 8: kernel accesses for a thread are
   checked against that thread's watches; a kernel write from outside the process needs the hook to name
   no thread (decision 10).
9. **Small accuracy problems.** Fixed: 155 lines in 29 files; the "3.2 ms for 48 MB" figure is gone with
   the argument it served; the kernel as a WebAssembly module with several callers (4.2, 7.6);
   `Config::shared_memory` named in 7.2 and T2's first task.

## What was read

- The brief and the interface it points to: wcontainers-scratch/orchestrator/brief-threads.md to its end,
  and next-embedder-interface.md with its later sections taken over its earlier ones.
- The results the owner has seen: orchestrator/dsm-survey/results.json (the 2008 system's cost of a
  recorded page hand-over), orchestrator/optimistic-replay-results.json (what ordering only atomics and
  kernel entries misses, what a single sequence costs, and which programs rely on barriers),
  wcontainers-scratch/ordering-cost/result.json (atomic against plain accesses, shared against unshared
  memories).
- This branch at 77e50ddc: docs/runtime.md whole; the kernel's entry points, scheduler, processes and
  signals (kernel lib.rs, task.rs, schedule.rs, process.rs, signal.rs, parts of memory.rs, shared.rs,
  fs/mod.rs, loader.rs, container.rs, syscall/system.rs, syscall/rseq.rs, fs/proc.rs); the abi; the
  runtime crate whole; the WebAssembly executor's types, `Execution::run`, `flush`, `block_write`,
  `settle` and outline (wasmtime lib.rs); the translator's contract, its memory imports, stop check,
  permission check and lookup; ARM64's atomics, exclusives and how barriers are decoded (arm64/memory.rs,
  decode.rs, instruction.rs).
- Other branches: docs/native.md at origin/phase-2-native ab448e58, sections 3.4 (the routes for nothing,
  aligned and exclusive forms), 5.1, 5.3, 5.5 and 11; docs/phase-2-guest-cpu-and-x86.md at
  origin/phase-2-x86 0de30f18, its lines on lock prefixes, on protecting code pages and its section 8. It
  does not name `mfence`.
- wasmtime 50.0.1's source in the cargo registry, for how a shared memory is made and switched on
  (src/runtime/vm/memory/shared_memory.rs:34-62, src/runtime/vm/memory.rs:251-256,
  src/runtime/instance.rs:554, src/config.rs above `shared_memory`), and its compiler's rule for
  `atomic.fence` (cranelift-codegen 0.137.1, src/isa/aarch64/inst/emit.rs:1809-1811,
  src/isa/x64/lower.isle:3425-3426).

- For the second check's findings, on this branch: the WebAssembly executor's loop, stop hook,
  `carry_on` and `leave`, `load_code`, `flush` and `block_write` (wt/lib.rs:2235-2340, :1780-1820,
  :1719-1740, :1885-1981, :1341-1433); `StopFlag` and the loop (rt/lib.rs:33-74, :190-300); the local
  system (rt/system.rs); `pick`, `came_back` and the tick (kernel/task.rs); `sched_setaffinity`
  (kernel/syscall/system.rs:161-190); `membarrier` (kernel/syscall/rseq.rs); `rseq_preempted`
  (kernel/signal.rs:648-665); the futex calls that change a guest word (kernel/syscall/futex.rs); the
  translator's contract and how a piece learns about its frame (tr/contract.rs, tr/full.rs:730-864,
  docs/phase-1-translator.md "Checks the translator drops"); the x86 plan's lines 1875-1915. The brief
  and the interface notes were read to their ends again: the brief's last section corrects `Settings`
  to `Config` and names `run`, and the notes' last sections give `run_until_finished` and `StopReason`.

- For the third check's findings: the brief's last three sections and the interface notes' last two;
  wcontainers-scratch/orchestrator/threads-plan-open-findings.json and progress-count-cost.json; the
  translator's contract, its stop check, how a guest hook is emitted and how loops keep their known
  pages (tr/contract.rs whole, tr/full.rs:1105-1160, :2110-2180, :2553-2569, :2746-2750,
  tr/code.rs:680-700, :1150-1210, :1285-1310, tr/thread.rs:110-140, guest-cpu/effects.rs:100-175); the
  executor's clock and stop hooks (wt/lib.rs:2095-2140, :1780-1820); docs/runtime.md sections 7, 9 and
  12 on the cache key, the progress count and replay; cranelift-codegen 0.137.1's alias analysis and
  where it is called (src/alias_analysis.rs:1-125, src/egraph/mod.rs:540-580, src/context.rs:180-200)
  and its setting (cranelift-codegen-meta 0.137.1, src/shared/settings.rs:60-68); docs/native.md at
  ab448e58 on barriers, invalidation and threads (lines 348-362, 987, 1028-1064). The native and x86
  branches have moved since the plan pinned them (origin/phase-2-native is at a7099033 and
  origin/phase-2-x86 at cbbb05ee); the plan's line numbers stay those of the pinned commits.

- For the fourth check's findings: the brief and the interface notes to their ends again (nothing in
  them is newer than what the third correction followed); how a process's signal picks its thread
  and every place a thread's mask is written (kernel/signal.rs:457-492 and the nine lines listed in
  the plan's 4.5); the hand-out, its slice and the tick (kernel/task.rs:17-104); the end of an entry
  and how a thread is woken and put in line (`settle`, `wake_thread`, `make_runnable`,
  `check_preempt`, kernel/task.rs:341-404, :519-582); the entry points (kernel/lib.rs:300-360); the
  loop and the local system's timer thread (rt/lib.rs:100-300, rt/system.rs:1-140, :230-300); the
  executor's run loop and its `rerun` path (wt/lib.rs:1719-1745, :2235-2345) and `code_line_changed`
  (:2008-2035); how a piece keeps what it knows about its frame, and across which calls
  (tr/contract.rs:1-27, tr/full.rs:725-870, :1310-1345, :2825-2910; docs/phase-1-translator.md,
  "Checks the translator drops" and "Go to address"); the x86 plan's rule for code on a live stack
  (its lines 1880-1905); docs/runtime.md section 10 for what leaving `run` costs; docs/native.md at
  ab448e58, section 5.5, for its timer; cranelift-codegen 0.137.1 src/alias_analysis.rs:44-70 and
  src/egraph/mod.rs:545-580; Linux 6.12's kernel/signal.c (fetched from its public source, read at
  lines 994-1035 and 2966-3082, and deleted again); the fourth checker's files in
  wcontainers-scratch/threads/check4/independent.

- For the fifth check's findings: the brief to its end again (its last section, "For the plan's last
  round", is new) and the interface notes to their end (nothing newer); the findings' file; the
  interface track's measurement of what a return of `run` costs (orchestrator/
  interface-measurements.json, the two "run-return" parts, with their method and what they did not
  measure); the hand-out, its slice, the tick and how a thread is put in line and compared with the
  running one (kernel/task.rs:17-105, :326-340, :343-407, :438-475, :519-584;
  kernel/schedule.rs:198-238); the entry points and what each does about timers
  (kernel/lib.rs:300-415); where the kernel shows a block in a memory (kernel/shared.rs:465-495) and
  the calls that map (kernel/syscall/memory.rs:109, :949, ipc.rs:356); the executor's copies at a
  thread's leaving and `block_write` (wt/lib.rs:1338-1435); the `System` trait as it is
  (rt/lib.rs:145-166); `lock_api`'s `RawMutex` and `GetThreadId`, as item 15's program uses them;
  ECMAScript's `IsNoTearConfiguration` (25.1.3.13, fetched from tc39.es and deleted again) and its
  rule "Tear Free Reads" (29.7.3); the WebAssembly threads proposal's overview (its repository,
  proposals/threads/Overview.md), which has no statement on a plain access being seen half done; the
  fifth checker's log (wcontainers-scratch/threads/check5/hostmem/run-mac.log).
  - Two of these were read through a tool that summarises a fetched page (the proposal's overview
    and the "Tear Free Reads" rule). `IsNoTearConfiguration`'s own text was read directly. The Arm
    and Intel manuals are cited in 7.6 for what a single aligned access promises; they were not
    opened for this correction.

## What the first agent left

No docs/threads.md was in the checkout, committed or not. In wcontainers-scratch/threads it left a clone of
this branch (`exp/`) with an experiment half made: an environment variable `WC_SHARED_GUEST=1` that makes
the translator import the guest memory shared and the translator's test runner supply a wasmtime shared
memory, a test `tests/spin_hoist.rs`, a 774 MB build, and copies of the native and x86 plans. The
experiment code was sound and its build was complete, so it was used as it was, with two small additions
to the spin test (it writes its modules as files; the declared maximum can be set).

## What was measured

Machines:
- this Mac: Apple M5 Max, 18 cores, macOS 27.2, with other tracks running. Load is `sysctl -n vm.loadavg`,
  the 1 minute figure;
- the x86 laptop: Intel Core i7-8750H, 12 threads, Linux 7.3.4, nearly idle. Load is `/proc/loadavg`, the
  1 minute figure. Everything ran under `taskset` on cores 4, 5, 6, 10 and 11; cores 2 and 8 were not
  used. Its files are in ~/threads-plan. No system package was installed: Node 24.22.3 and wasmtime
  49.0.2 were already in the home folder from earlier measurements.

Everything is under wcontainers-scratch/threads. Build output was deleted afterwards on both machines.

### First run

**1. A shared guest memory against an unshared one, single-threaded.** `shared-cost.sh`, run inside `nix
develop <this checkout> -c`. It runs the translator's benchmark test (`measure_benchmarks`,
crates/wcontainers-translator-test/tests/benchmarks.rs, `programs/bench.c` at -O2, full look-ahead, every
check, wasmtime 50.0.1 only) once per workload per setting under `/usr/bin/time -l`, alternating the two
settings. Every run checked the program's output against the native run's.
- Three rounds with one run in each process: logs/shared-cost.log, load 49 to 64.
- Two rounds with three runs in each process: logs/shared-cost-runs3.log, load 72 to 96.
- The whole process's instructions include fixed work that is not the translated run (9.3 to 9.4 thousand
  million instructions in every case). The difference between the three-run and one-run processes, halved,
  is one translation, compile and run. In thousand millions of instructions, unshared then shared: fib
  81.602 and 81.580 (-0.03%); sieve 14.038 and 14.049 (+0.07%); hash 7.959 and 7.967 (+0.10%); sort 31.710
  and 31.733 (+0.07%); interpret 22.896 and 22.871 (-0.11%); matrix 7.967 and 7.960 (-0.09%); walk 16.790
  and 16.763 (-0.16%); search 6.947 and 6.969 (+0.33%).
- The two rounds of one setting differed by up to 0.39% (search, shared: 6.956 and 6.983) and 0.19% (walk,
  unshared). So no difference is shown.
- Whole-process counts from the one-run rounds, mean of three, shared against unshared: fib -0.02%, sieve
  -0.23%, hash -0.12%, sort -0.10%, interpret +0.16%, matrix -0.04%, walk +0.04%, search -0.23%.
- Best time of three, unshared then shared, in seconds: fib 2.806 and 2.476; sieve 0.706 and 0.681; hash
  0.591 and 0.589; sort 1.322 and 1.308; interpret 0.623 and 0.653; matrix 0.274 and 0.302; walk 2.733 and
  2.576; search 0.258 and 0.260. Times at this load vary more than these differences.
- What this does and does not cover: the import's type and a wasmtime `SharedMemory` made by wasmtime
  itself, in the translator's runner. It does not cover the runtime's host-supplied mapping as a shared
  memory, one store per thread, anything with two threads, or any program with a barrier or a 128 bit
  atomic in it.

**2. A loop of plain loads sees another thread's store.** `tests/spin_hoist.rs` in the scratch clone, and
`spin-node.mjs`. One thread runs a loop that counts and loads a 64 bit word from a shared 64 bit memory
until it is not zero. Another thread stores 1 to the word after 200 ms (wasmtime) or 300 ms (Node). Three
loops: a bare plain load; a plain load with the translator's stop check in the loop (a plain load of a
flag in a second shared memory and a call when raised); an atomic load.
- wasmtime 50.0.1 (logs/spin-wasmtime.log, load 69): all three saw the store, after 755 million, 439
  million and 465 million rounds, at 201 to 205 ms.
- Node 24.14.1 (logs/spin-node.log, load 62 to 64): all three saw it with the default compilers, with
  `--no-liftoff` (the optimising compiler alone) and with `--liftoff-only` (the baseline compiler alone).
- This shows what these versions do. WebAssembly's rules do not promise it, so the plan makes it a test
  of every version of each WebAssembly compiler (docs/threads.md section 12).

**3. V8 refuses a declared maximum above its limit.** The first Node run of experiment 2 failed to
compile: "maximum memory size (281474976710656 pages) is larger than implementation limit (262144 pages)".
The module imported a shared 64 bit memory with a declared maximum of 2^48 pages. With a declared maximum
of 262,144 pages it compiled and ran. wasmtime accepted both.

### By the plan's checker

**4. The message-passing test.** check1/mp.mjs, logs mp-node.log and mp-node-2.log; this Mac, Node 24.14.1,
load 99 to 107. Two workers, plain 32 bit accesses on a shared memory. The flag was seen with stale data
84 times in 1,500,000 rounds and 64 times in 2,000,000. With `atomic.fence` between the two stores and
between the two loads: 0 times in 3,500,000. In one thread, a fence between two plain stores took a round
from about 0.35 ns to about 1.6 ns.

### Second run, for the corrections

**5. A barrier in a single-threaded run, in three forms.** barrier/bench.wat, assembled with `wasm-tools
parse` from the checkout's `nix develop`. A module with a shared 64 bit guest memory and a shared 32 bit
state memory, as the translator's contract has them. A round stores a word, has the barrier, and loads
another word. Four exported loops: `none` (the barrier is nothing, as today); `fence` (`atomic.fence`);
`flag0` and `flag1` (a byte of the state memory is loaded, and `atomic.fence` runs when it is not zero;
the byte is 0, then 1). Nanoseconds a round, best of five.
- This Mac, Node 24.14.1, `node bench.mjs bench.wasm 200000000 5` (barrier/bench-node-mac.log, load 90 to
  91): none 0.463, fence 6.590, flag0 0.585, flag1 6.907.
- This Mac, wasmtime 49.0.2's command line program from `nix develop`, `bench-wasmtime.sh`, 500,000,000
  rounds, each time less the time of a run of no rounds (barrier/bench-wasmtime-mac.log, load 90 to 91):
  none 0.975, fence 1.672, flag0 1.146, flag1 1.739.
- The laptop, Node 24.22.3, core 4 (barrier/bench-node-x86.log, load 0.2 to 0.9): none 1.051, fence
  17.047, flag0 1.233, flag1 17.577.
- The laptop, wasmtime 49.0.2, core 4 (barrier/bench-wasmtime-x86.log, load 0.9 to 1.2): none 1.763,
  fence 20.343, flag0 2.563, flag1 20.121.
- So the fence costs 0.7 and 6.1 ns a barrier on this Mac and 18.6 and 16.0 ns on the laptop. The byte's
  test costs 0.17 and 0.12 ns on this Mac and 0.80 and 0.18 ns on the laptop.
- wasmtime and Node differ on this Mac for the fence (0.7 against 6.1 ns). wasmtime's loop is slower to
  begin with (it calls a function for the address and checks bounds); why the fence then costs less there
  was not looked into.
- wasmtime's compiled output on this Mac was disassembled (`wasmtime compile`, then `objdump -d`): the
  fence is `dmb ish`, and the tested form is `ldrb`, `cbz` around `dmb ish`.
- Not covered: wasmtime 50.0.1 (the nix shell has 49.0.2; the compiler's rule for the fence was read in
  50.0.1's own compiler source instead); Chrome; the translator's real output, where the byte's place and
  the register holding its base may differ.

**6. The store-then-load test.** barrier/litmus.wat and litmus.mjs, in Node with two workers on one shared
memory. Each round has its own two words. The threads meet at an atomic counter, then thread 0 stores to
x and loads y, and thread 1 stores to y and loads x. "Both loads saw 0" is forbidden when a full barrier
stands between each thread's store and its load. Four forms of the store: plain; plain then
`atomic.fence`; an atomic store; an atomic exchange. `node litmus.mjs litmus.wasm 200000 10`, so 2,000,000
rounds of each.
- This Mac, Node 24.14.1 (barrier/litmus-node-mac.log, load 89 to 91): 371,914; 0; 0; 0.
- The laptop, Node 24.22.3, cores 4, 5, 6, 10, 11 (barrier/litmus-node-x86.log, load 1.2): 134,938; 0; 0;
  0.
- So on both hosts plain accesses show the outcome, which is why a guest's barrier cannot be nothing, and
  `atomic.fence` forbids it. An atomic store or exchange in the barrier's place forbade it too, in V8.
- Not covered: wasmtime with two threads (its command line program runs one); a test through the
  translator.

**7. An uncontended lock.** lock/src/main.rs, a small Rust program with `parking_lot` 0.12.5 through
`lock_api` 0.4.14, release build. One thread; a round is two small pieces of work; a `parking_lot` raw
lock is taken and let go around none, one or both. Nanoseconds a round, best of five, 300,000,000 rounds.
- This Mac, built with `cargo build --release --offline -j 3` under `nix develop` (lock/lock-mac.log, load
  83 to 96): 4.863, 5.141, 4.977. The lock cannot be told from the noise.
- The laptop, core 4 (lock/lock-x86.log, load 0.6 to 1.7): 5.514, 12.596, 21.540. So 7.1 and 8.9 ns a
  lock.
- Not covered: the lock inside a real system call, where the work around it is different.

### By the plan's second checker

**8. An uncontended lock, with two kinds of work around it.** check2/lock/lock.c, a C program: the lock is
`parking_lot`'s fast path written out, a compare and swap to take it (acquire) and one to let it go
(release). "Register work" is a dependent multiply that touches no memory. "Memory work" walks a chain
of eight loads in a 64 KiB table and stores once. Best of five, 100,000,000 rounds, nanoseconds a round
without the lock and with it.
- This Mac (check2/lock/lock-mac.log, load 53 to 71): register work 0.950 and 4.523; memory work 8.421
  and 8.414.
- The laptop, core 4 (check2/lock/lock-x86.log, load 5.5 to 5.7): register work 2.667 and 14.041; memory
  work 16.852 and 17.485.
- So a lock costs 0 to 3.6 ns on this Mac and 0.6 to 11.4 ns on the laptop, by the work around it. Item
  7's figure for this Mac was the loop's, not the lock's. What it costs inside a real system call is
  measured only by T1.

**9. The store-then-load test in wasmtime with two host threads.** check2/host (a Rust program on
wasmtime 50.0.1 as the repository pins it, `Config::wasm_threads` and `Config::shared_memory`), with
check2/host/src/sb.wat. Two host threads, each with its own `Store` and `Instance`, on one wasmtime
`SharedMemory`. Twenty runs of 100,000 rounds for each form; the count is rounds in which both loads saw
0.
- This Mac (check2/sb-mac.log, load 102 to 106): plain accesses 30,199; `atomic.fence` 0; the byte
  tested and zero 30,425; the byte tested and set 0; a compare and exchange then a plain load 0; an
  atomic store then an atomic load 0.
- The laptop, cores 4 and 10 (check2/sb-x86.log, load 8.8 to 11.3): 143,587; 0; 39,671; 0; 0; 0.
- So item 6's "not covered: wasmtime with two threads" is now covered for this test. Build output was
  deleted on both machines.

### Third run, for the third check's findings

Everything is under wcontainers-scratch/threads/check4, with noalias-cost.sh and logs/noalias-cost.log
beside it. The laptop was in use by other tracks during these runs (a Chrome benchmark and two other
experiments were running), so its load was 5 to 9 and its timings are noisier than the first run's.
Counts are not affected.

**10. What wasmtime's compiler does to plain accesses, read in its output.** check4/shapes.wat, six
functions on a shared 64 bit memory, compiled with wasmtime 49.0.2's `wasmtime compile` from `nix
develop` on this Mac (load 110 to 113) and read with `wasmtime objdump`
(shapes-mac-wasmtime49-default.dis).
- "Load x, load y, load x": two guest loads; the first is used twice.
- The same with each load atomic when its address is aligned and plain when not: all three loads kept,
  in both arms.
- "r = [x]; f = [y]; if f then [x] = r": no store in the function. The same with no branch: no store.
- The same with the middle load atomic: the store is there.
- "[x] = v; f = [y]; [x] = v": one store.
- Compiled again with `-C cranelift-enable_alias_analysis=false`: `diff` of the two disassemblies is
  empty (shapes-mac-wasmtime49-noalias.dis). In cranelift-codegen 0.137.1 the setting is defined
  (cranelift-codegen-meta src/shared/settings.rs:60-68) and nothing in cranelift-codegen's source reads
  it (`grep -rn enable_alias_analysis src` finds only the settings test's listing).

**11. The same with two threads.** check4/host: a Rust program on wasmtime 50.0.1, two host threads,
each with its own `Store` and `Instance`, on one shared 64 bit `SharedMemory`; the module is
check4/host/src/tso.wat. It is built against a scratch copy of cranelift-codegen 0.137.1
(wcontainers-scratch/threads/cranelift-codegen-patched; the change is
check4/cranelift-skip-alias-analysis.patch): with `WC_NO_ALIAS=1` in the environment the optimiser
treats every load and store as it treats one the analysis has nothing to say about. Without the
variable the copy behaves as the released crate. Built with `cargo build --release --offline`, two jobs
on this Mac and five on the laptop (cores 3, 4, 5, 9, 10, 11).
- *Sequence lock.* A writer makes a count odd, writes two words, makes the count even, all plain stores.
  A reader loads the count, the two words and the count again, and accepts when the count was even and
  unchanged. Three readers: plain loads; each load atomic when aligned; atomic loads. The figure is
  accepted reads whose two words differed, of all accepted reads.
  - The laptop, cores 4 and 10, 20,000,000 writes (tso-x86.log, load 7.7). Pass on: 931,346 of
    1,670,130; 0 of 3,692; 0 of 21,038. Pass off: 0 of 33,327; 0 of 7,535; 0 of 20,804.
  - The laptop, 200,000,000 writes (tso-x86-long.log, load 5.3). Pass on: 9,288,492 of 15,869,202; 0 of
    24,783; 0 of 98,511. Pass off: 0 of 109,791; 0 of 19,103; 0 of 120,155.
  - This Mac, 20,000,000 writes (tso-mac.log, load 129). Pass on: 7,784 of 3,311,008; 20 of 1,821,534; 1
    of 237,583. Pass off: 82,860 of 4,204,412; 7 of 131,353; 2 of 113,583. Every form fails on this
    host: it reorders the writer's plain stores, and plain loads too. That is the weaker-host case of
    the plan's section 9, where x86 guests are kept `Serial`.
  - The correct readers accept few reads because the writer never rests; the longer run was made to
    have more of them.
- *Store-back.* Thread 0: r = [x]; a wait of 200 empty rounds; f = [flag]; if f then [x] = r. Thread 1:
  [x] = 1; `atomic.fence`; [flag] = 1. The two meet before and after each round. The figure is rounds
  with r = 0, f = 1 and x = 1 at the end, of rounds with r = 0 and f = 1.
  - This Mac, 200,000 rounds (load 129). Pass on: plain loads 57,368 of 57,368; loads atomic when
    aligned 0 of 41,951; atomic loads 0 of 120,649. Pass off: 0 of 61,447; 0 of 28,571; 0 of 65,136.
  - The laptop, 200,000 rounds (load 7.7). Pass on: 199,975 of 199,975; 0 of 199,625; 0 of 199,882. Pass
    off: 0 of 199,977; 0 of 199,911; 0 of 199,874. With 1,000,000 rounds (load 5.3): pass on 999,877 of
    999,877; pass off 0 of 999,730.
- Why the outcome is forbidden. Thread 1's store to x is before its store to flag, with a full barrier
  between. Thread 0 stores to x only after it has seen the flag, and a store is not made before the
  branch it is under is decided. So thread 0's store is the later one and x ends as r. The compiler
  leaves the store out because r is "what x holds".
- Not covered: the same shapes through the translator; V8.

**12. What that pass is worth to a single-threaded run.** noalias-cost.sh, on this Mac inside `nix
develop <this checkout> -c`. The translator's benchmark test in the scratch clone (`exp/`, the first
run's, with the patched Cranelift copy, a shared guest memory by `WC_SHARED_GUEST=1`, and a count of
the instructions the process retired during the run alone, read with `proc_pid_rusage` before and after
`Machine::run`). Each workload in its own process, the pass on and off alternating, three rounds, full
look-ahead, every check, wasmtime 50.0.1. Every run's output was checked against the native run's.
Load 123 to 142 (logs/noalias-cost.log).
- Thousand millions of instructions, mean of three, on then off: fib 81.513 and 81.948 (+0.53%); sieve
  13.842 and 13.844 (+0.01%); hash 7.801 and 7.804 (+0.04%); sort 31.437 and 31.439 (+0.01%); interpret
  22.512 and 22.734 (+0.99%); matrix 7.716 and 7.725 (+0.11%); walk 16.578 and 16.879 (+1.82%); search
  6.777 and 6.778 (+0.00%). Geometric mean +0.44%.
- The three runs of one workload and one setting differed by 0.02% to 0.10%, so the three larger
  differences are real and the rest are not shown.
- Times at this load vary far more than this (best of three, on then off, in seconds: fib 2.883 and
  2.791; walk 3.315 and 3.026) and say nothing.
- Not covered: an x86 host; programs larger than the benchmark set; what the pass costs to compile.

**13. Atomic loads in place of plain ones, single-threaded, on an x86 host.** check4/kernels: the
generator of wcontainers-scratch/ordering-cost with two forms added, "loads" (atomic loads, plain
stores) and "lchk" (each load's address tested for alignment: atomic when aligned, plain when not; plain
stores), on a shared 64 bit memory. The laptop, wasmtime 49.0.2, `taskset -c 4`, seven rounds with the
forms alternated, best of seven (kernels/bench-x86.log, load 5.2 to 7.0). Checksums agreed in every
form.
- Seconds for plain, atomic loads, tested loads: sum of 64 bit words 1.253, 1.855, 2.467; pointer chase
  1.354, 1.366, 1.395; byte scan, copy and 16 bit sum 2.239, 2.170, 2.526; recursive calls with a frame
  in memory 1.225, 1.217, 1.533; fill and copy 0.428, 0.438, 0.477; the copy with both buffers one byte
  off alignment 0.435 plain and 0.449 tested.
- So atomic loads cost 0.97 to 1.48 times and tested loads 1.03 to 1.97 times. Byte loads have no
  alignment to test, which is why the byte kernel does not move with atomic loads.

Build output was deleted afterwards on both machines (the scratch `target` folders and the compiled
modules); sources, logs and the patched crate's source are kept.

### By the plan's fourth checker

**14. Whether the compiler's pass drops an overwritten store.** check4/independent/dse.wat, five
functions on a shared 64 bit memory, compiled with wasmtime 49.0.2's `wasmtime compile` from `nix
develop` of this checkout on this Mac (load 101 to 109 by the checker's note) and disassembled
(dse-default.dis), then again with `-C cranelift-enable_alias_analysis=false` (dse-noalias.dis).
- "[x] = 1; [y] = 2; [x] = 3" and "[x] = 1; f = [y]; [x] = 3", each with the addresses in two
  registers and again as constant offsets from one base: all ten stores are in the output.
- "[p]; [p+8]; [p]": the repeated load is merged into one, as item 10 found by register.
- The two disassemblies are the same, line for line, as in item 10.
- For this correction the two .dis files were read again (ten store instructions; `diff` empty). It
  was not compiled again, and nothing was run with two threads: there is no outcome to look for
  while every store is kept.
- So the pass's dead-store elimination does not reach guest stores in this version. Its header says
  why: an instruction that may trap counts as seeing every store, and every access to the guest
  memory may trap.

### Fourth run, for the fourth check's findings

Nothing was built or run. Every finding was a matter of the plan's own rules against the code, and
each was settled by reading (the list is under "What was read"). No scratch files were added and
there was no build output to delete.

### Fifth run, for the fifth check's findings

Everything is in wcontainers-scratch/threads/run6, and on the laptop in ~/threads-plan/run6. Build
output was deleted on both machines; sources, the assembled module, the listings and the logs are
kept. Builds used three jobs at most. On the laptop no system package was installed, and cores 2 and
8 were not used.

**15. The holder's word on an uncontended lock** (for finding 5). run6/owner/src/main.rs: a
`parking_lot` 0.12.5 raw lock through `lock_api` 0.4.14, around a small piece of work, in one thread.
Four forms of a round: no lock; the lock; the lock with the holder's name read
(`parking_lot::RawThreadId` through `lock_api::GetThreadId`) and stored in an atomic word after the
lock is taken, and the word cleared before it is let go; the same with the name read once outside the
loop. Two kinds of work, as the second checker's item 8: one that stays in registers, one that walks
a chain of eight loads in a 64 KiB table and stores once. 200,000,000 rounds, best of seven, the
forms taken in turn inside each repeat. Built with `cargo build --release --offline -j 3`, on this
Mac under `nix develop` of this checkout.
- This Mac (owner/owner-mac.log, load 133 before and 137 after), nanoseconds a round for the four
  forms: register work 2.022, 5.515, 5.012, 5.917; memory work 8.742, 9.138, 9.171, 9.107.
- The laptop, `taskset -c 4` (owner/owner-x86.log, load 4.7 before and 6.1 after): register work
  1.808, 11.809, 11.569, 12.067; memory work 13.889, 14.521, 14.764, 14.374.
- So the lock itself costs 0.4 to 3.5 ns on this Mac and 0.6 to 10.0 ns on the laptop here, inside
  the ranges of items 7 and 8. The word changes a round by -0.50 to +0.40 ns on this Mac and by
  -0.24 to +0.26 ns on the laptop, with both signs in each kind of work. No cost is shown.
- What it does not cover: the lock inside a real kernel entry, the name of a virtual CPU inside the
  native executor's machine, or the kernel built as a WebAssembly module. Run once on each machine.

**16. Whether an aligned plain 64 bit access is seen half done** (for finding 7). run6/tear/tear.wat,
assembled with `wasm-tools parse` from the checkout's `nix develop`: a module importing a shared 32
bit memory. `writer(n, at)` stores 0 and -1 in turn to the 64 bit word at `at` with plain stores, one
store each time round. `reader(n, at)` loads the word with a plain load n times and counts the values
that are neither; it also counts the loads that saw -1, so a run that never saw both values can be
told. Two forms: `at` aligned to 8 (4096), and `at` four bytes before a 64 byte boundary (4156), a
control that shows the test can see a mix. 200,000,000 rounds a run, five runs a form, so
1,000,000,000 loads.
- Node, two workers on one `WebAssembly.Memory` (tear.mjs). This Mac, Node 24.14.1
  (tear-node-mac.log, load 112 to 113): aligned 0 mixed; across a line 73,917,899. The laptop, Node
  24.22.3, `taskset -c 5,10` (tear-node-x86.log, load 5.2 to 6.3): 0; 232,854.
- wasmtime 50.0.1 as the repository pins it, two host threads each with its own `Store` and
  `Instance` on one `SharedMemory` (tear/host, built in release with three jobs). This Mac
  (tear-wasmtime-mac.log, load 113 to 114): 0; 57,220,868. The laptop, `taskset -c 5,10`
  (tear-wasmtime-x86.log, load 3.3 to 3.8): 0; 37,550,933.
- In all eight runs the reader saw both values: between 617 and 937 million of its loads saw -1.
- wasmtime 49.0.2's output for the module, from `wasmtime compile` and `wasmtime objdump` on each
  host (tear-mac-wasmtime49.dis, tear-x86-wasmtime49.dis): the store is the one instruction `str x13,
  [x10, w5, uxtw]` and the load `ldr x1, [x0, w5, uxtw]` on this Mac; `movq %rdi, (%rax, %rcx)` and
  `movq (%rdi, %rcx), %r9` on the laptop.
- What it does not cover: V8's machine code, which was not read; a 64 bit guest memory, where the
  address is made differently and the access is the same instruction; an access the translator
  emits, since the module is hand-written; any other version of either compiler. The plan makes it
  a test of each version (docs/threads.md section 12).

### By the plan's sixth checker

**18. Two call-ins that want each other's groups, as a model.** check6/callin_model.py and its two
logs (Python 3, this Mac, load about 118 by the checker's note). A model of 4.3's and 4.5's rules as
they were at abfc95e2, searched breadth-first: P1 with threads A1 and S1, P2 with A2, all `Serial`,
two processors; A1 and A2 each make one call that reaches the other's group; any thread may give way
or end. It reaches a state where nothing can move after four steps, having seen 67 states
(callin_model.log). With `--control`, where a waiter is put forward whatever its own group's mark
says, no such state exists in 106 states (callin_model-control.log). Read and run again for this
correction, with the same result.

### Sixth run, for the sixth check's findings

Everything is in wcontainers-scratch/threads/run7, and on the laptop in ~/threads-plan/run7. Build
output was deleted on both machines; sources and logs are kept. Builds used three jobs at most. On
the laptop no system package was installed, and cores 2 and 8 were not used.

**19. The corrected call-in rules, as a model** (for finding 1). run7/callin/callin_model2.py and
callin_model2.log; Python 3.15.0 with `-I`, this Mac at load 117 to 124 for the logged runs (134 to
154 for a first set, which gave the same counts); each search takes seconds.
What is modelled, as 4.3 and 4.5 now state it:
- a call names a set of groups, asks their running threads in and parks; it is woken when none of
  them has a thread in guest code; a group is called in while a pending call names it;
- a waiter is put forward whatever is called in; in a `Serial` group only when none of its threads is
  in guest code, and before the group's other threads; a `Parallel` group puts forward every ready
  thread, and only its waiters while it is called in;
- when its call has run, the thread goes back to guest code if its group is not called in, and stays
  ready otherwise;
- the kernel asks in a thread that stands in a waiter's way;
- any thread may make its one call, come in, or end; a thread that waits for a call-in may be ended,
  and its call-in goes with it.
Two cases. `check6` is the checker's own. `three` is two `Serial` groups of two threads and one
`Parallel` group of two, where each of four threads makes one call chosen freely from a menu:
reaching another group, reaching two, joining its own group to another, or calling its own group in.
What is checked, and what was found:
- states reached: `check6` 156; `three` 4,998, 55,053, 60,743 and 62,189 on 1, 2, 3 and 4 processors;
- no state where a thread has not ended and no step is possible;
- from every state the state where every thread has ended can be reached;
- the steps assert, and none failed: never two threads of a `Serial` group in guest code; a call's
  work only while no thread of a group it names, or of its own `Serial` group, is in guest code; a
  thread enters guest code only when its group is not called in; a woken call never has to park
  again;
- with only the steps the kernel forces (an asked thread comes in; a waiter is chosen when a
  processor is free; a thread in a waiter's way is asked in): from every state every path ends with
  no call-in pending, the longest in 9 steps, and none goes round;
- with `--old`, the rule before this correction: in `check6` two states, and in `three` 656, end with
  a call-in pending and nothing forced left to do. Those are the checker's stuck state and its
  kin. This model lets a waiting thread be ended, which the checker's does not, and that step is the
  only one left in those states. With it left out (`--no-kill`) the earlier rule reaches a state
  where nothing can move, in four steps in both cases, and the rule now reaches none (137 states in
  `check6`, 53,526 in `three` on two processors), with every check above still passing.
What it does not cover: masks, weighted time, more than six threads, a call that parks for
something else after its call-in, the merging of two groups after a join (a join is modelled as a
call that names both groups, and the groups stay apart afterwards), and the code. Section 12's test
is for the code.

**20. A barrier as a hardware instruction, single-threaded** (for finding 6).
run7/native-barrier/barrier.c: a round is a store to one word, the barrier, a load of another word,
both words `volatile`; 200,000,000 rounds, best of seven, the forms taken in turn inside each repeat.
At user level, with no virtual machine.
- This Mac (`cc -O2 -march=armv8.1-a`, Apple clang 21.0.0; barrier-mac.log, load 158 before and 162
  after), nanoseconds a round: nothing 0.24; `dmb ishld` 1.46; `dmb ishst` 1.47; `dmb ish` 1.56; `dsb
  ish` 5.02; `isb` 8.40; an atomic add in place of the barrier 1.67. `otool -tv` shows the barrier
  instructions in the program.
- The laptop (`gcc -O2`, gcc 9.4.0, `taskset -c 4`; barrier-x86.log, load 2.6): nothing 0.51;
  `mfence` 17.69; a locked add in place of the barrier 6.43. `objdump -d` shows them.
- So a copied barrier would cost a `Serial` run 1.2 to 8.2 ns each on this Mac and `mfence` 17.2 ns
  on the laptop. `mfence` here agrees with the 19 ns that `atomic.fence` cost in wasmtime on the
  laptop (item 5).
- What it does not cover: the same instructions at the guest's level inside the native executor's
  virtual machine (nothing traps there, so the same is expected), `sfence` and `lfence`, and any
  other processor. Run once on each machine.

**21. A per-thread flag tested at every take of an uncontended lock** (for finding 4). run7/flag: item
15's program with a fifth form, which is its third form (the lock, the holder's name read and stored)
with a `thread_local!` `Cell<bool>` read and tested before the lock is taken, and a cold function
that panics behind the test. Same rounds, repeats and kinds of work. Built with `cargo build
--release -j 3`, on this Mac under `nix develop` of this checkout.
- This Mac (flag-mac.log, load 126 before and 143 after), third form and fifth, nanoseconds a round:
  register work 4.760 and 4.888; memory work 8.470 and 8.375.
- The laptop, `taskset -c 4`, three runs (flag-x86.log, flag-x86-again.log, load 1.6 to 2.5):
  register work 11.870 and 11.175, 11.758 and 11.143, 11.869 and 11.143; memory work 14.114 and
  15.104, 14.114 and 15.040, 13.959 and 14.953.
- So on this Mac the test cannot be told from the noise. On the laptop it moves a round by -0.6 to
  -0.7 ns with register work and by +0.9 to +1.0 ns with memory work, the same in each of three
  runs. A cost that changes sign with the work around it is most likely where the code landed, not
  the load itself; but it is of the lock's own size, so the plan keeps the test off the system call
  (4.2).

**22. How many virtual CPUs KVM allows one machine** (for finding 5). run7/kvm/maxvcpus.py:
`KVM_CHECK_EXTENSION` on /dev/kvm for three capabilities, with Linux's own numbers
(include/uapi/linux/kvm.h). The laptop, Linux 7.3.4, load 2.7 (maxvcpus-x86.log): `KVM_GET_API_VERSION`
12; `KVM_CAP_NR_VCPUS` 12; `KVM_CAP_MAX_VCPUS` 4096; `KVM_CAP_MAX_VCPU_ID` 16384. No virtual machine
was made. KVM on ARM64 was not read: no such machine is at hand.

### By the plan's fifth checker

**17. A host-supplied mapping as a wasmtime shared memory.** check5/hostmem (its source and
run-mac.log; wasmtime 50.0.1 as the repository pins it, upstream Cranelift, a debug build with three
jobs, this Mac at load 113 to 127 by the checker's note). The engine was set up as the executor's:
`wasm_threads`, `memory64`, `shared_memory(true)`, `memory_may_move(false)`, and `with_host_memory`
with a creator that records its calls. The log, read again for this correction:
- the creator was called once after the defining module was instantiated, for a shared 64 bit memory
  with a minimum of 0x10000 bytes and a maximum of 0xff000000 (64 KiB and 4080 MiB);
- `SharedMemory::data()` has the creator's own base, 0x7000000000;
- a store by a second thread's instance in its own store was seen through the host's mapping, and a
  host write was seen by a guest load in a third store;
- growing by three pages went through the creator's `grow_to`, once.
So the path the plan's 7.2 read in wasmtime's source works. It was not run in the executor, where
today's state memory is made by `SharedMemory::new` (wt/lib.rs:1242).

## What was not measured

Listed in docs/threads.md section 15. The ones that matter most to the plan's decisions:
- how long processors would wait for the kernel's one lock in a real build: it needs T1 built;
- what T1 adds to a system call end to end; only the lock by itself was measured (items 7 and 8), and
  it costs between nothing and 11.4 ns by the work around it, and its holder's word by itself (item
  15), which could not be told from the noise;
- what the scheduler's condition costs when it is walked, how often that is, and how many turns end
  for nobody because a slice is still worked out from the whole run queue: all three need T1 built;
- threaded programs run `Serial` once each thread has its own store: T2's bar measures them;
- how often real programs run barriers and 128 bit atomics: the cost of each is measured, their number
  is not;
- what switching the compiler's alias analysis off costs on an x86 host and on large programs, and
  whether V8 does what that pass does;
- what the digest of `External` costs, and what calling a group in for every change of the kernel's
  costs a (`Parallel`, `Exact`) group;
- the runtime's host-supplied mapping as a wasmtime shared memory inside the executor: it was run
  outside it by the fifth checker and worked (item 17);
- V8's machine code for a plain 64 bit access: not read. Node showed no mixed value (item 16);
- a real `suspend` and `resume`, and anything of them on the native executor: the interface track
  measured only the return of `run` at a program start, on a prototype;
- the ordering tests as guest programs through the translator: only hand-written tests were run (items
  4, 6 and 9);
- what the stop lock costs the kernel's accesses to a `Parallel` memory, and how often a translation is
  thrown away because its code was dropped meanwhile;
- a group stop with several threads running, and a page hand-over through per-thread watches, are both
  estimated from parts measured apart (the stop flag's latency in Chrome from docs/phase-1-translator.md,
  leaving `Execution::run` and coming back from docs/runtime.md section 10);
- how often a `Parallel` group takes a permission from a data page or unmaps one, which now makes
  each of its other running threads give up its frames, and what their returns through the bottom
  loop cost afterwards;
- a timer sent by the local system's timer thread while every processor is busy, and how many
  threads a real program's timers bring in. The design brings in one at most for each expiry;
- whether the frame rule's claim about a thread on a second stack shows in a run: it was read in the
  translator (tr/full.rs:2835-2853), not run. Section 12 has the test;
- the call-in rules in code: they were searched as a model (item 19), not built;
- what the question and the digest cost a process given `None` in a recorded kernel: T8 measures it;
- a container with more processors than its host has cores, where the host takes a worker off its
  core while it holds the kernel's lock: T1's build measures it;
- a barrier inside the native executor's virtual machine: item 20 is at user level on hardware;
- `KVM_CAP_MAX_VCPUS` on an ARM64 host.

## What the plan asks of other tracks

- wasmtime's project: a fix that makes Cranelift's `enable_alias_analysis` setting do what it says
  (check4/cranelift-skip-alias-analysis.patch is the experiment's form of it, switched by an environment
  variable; the real one reads the setting). Until a release has it, a patched `cranelift-codegen`
  (decision 4).
- The translator track: the progress count, now only in the code of a process in force `Exact`, and in
  both forms by ordering; the Parallel form (barriers and 128 bit atomics behind the byte, `isb` among
  the barriers); the mark for hooks that are stop points, with all registers saved before them and
  what the piece knew forgotten after them; a word on whether a check that covers several accesses can
  span a call or a jump out of the piece; `raise_stop` as an atomic or. The stop word's meaning to
  translated code does not change. Only if T2 or T3 measures it to matter: a bound on where a
  thread's live frames are, so that fewer threads give up their frames after a permission is lost
  (docs/threads.md 4.5).
- The x86 track: for the Parallel form, lock prefixes and `xchg` with memory as WebAssembly atomic
  operations, with a hook, made a stop point, for misaligned ones; `cmpxchg16b` as a 128 bit pair
  behind the byte; `mfence` as the tested fence; the sequence lock and the store-back among its
  ordering tests (docs/threads.md 7.3, section 9). Its Serial form does not change. Its rule for code on
  a live stack frame (its lines 1898-1905) tests by the thread's stack pointer and its mapping. That
  misses a thread that has moved to another stack inside a call. It should use the plan's test, by
  the page (a page that allowed reading and writing), and the plan's one piece of code for a thread
  that gives up its frames (4.5).
- The native track: two forms of native code by ordering, under the same cache key field as the
  WebAssembly output's: the Serial form emits nothing for `dmb`, `dsb`, `isb` and `mfence` and leaves
  lock prefixes out, the Parallel form copies them (section 10); this plan's calls on the inside half
  of its split of `System`, with a recording system's log kept inside and sent out in batches, and
  the mark for a guest hook kept for each virtual CPU (section 10, 4.2); the exclusive load's
  fix-up made safe for threads in parallel; each held virtual CPU dropping its own cached translations
  before it is let go; an interrupt from one virtual CPU to another inside the machine measured; a
  request to stop kept until the thread's next entry; a stop lock per memory and the count of drops;
  call-ins, not group stops, for a change of the kernel's in a group in force `Exact` (section 10);
  one timer inside for the container's deadline, armed from `on_deadline_changed`, beside its timer
  for slices (section 5); the virtual CPU's number as the name of the kernel's lock's holder (4.2).
  Nothing before its own thread milestone.
- The kernel track's K7: the kernel as a WebAssembly module built for a shared memory, for more than one
  caller, each instance with a name for the lock's holder (4.2).
- The embedder interface: `ExecutionConfig` with both fields at the container's start; `suspend` and
  `resume`; the config in force beside the config given, and the hook that announces its change
  (section 3, decision 1); memory watches before T4, with a thread's position readable while it is
  held; who shares memory with whom, and shared pages marked watched when one process of a group
  leaves; the hook's thread made optional (decision 9); `Kernel::process_call_in` and its hook, and
  `kernel.process(id)` at any time with the rule for which operations answer an error (decision 8);
  a `SuspendedProcess` that shares no `Rc` with what stays in the kernel, with the references it
  shares kept inside under a key and let go under the lock (4.2).
  No longer asked: `Process::replace_execution_config` (section 6).
- The runtime track: the renames to `RuntimeConfig`, `TranslatorConfig`, `run` and `run_until_finished`
  in the code, which this plan already uses; two wasmtime engines in the executor, and the cache key's
  two new fields (7.2); `Carry::Leave` carrying the kernel's handle (4.2); the doc comments of `ran`,
  the clock's answer and `cpu_warning` (4.2); one worker for each processor in `run_until_finished`
  (section 5).
