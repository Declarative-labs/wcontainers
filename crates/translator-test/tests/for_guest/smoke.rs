// Written once for both guests; each guest's module in `smoke.rs` includes it.

/// Load `code` at `LOAD` at `look_ahead`, every register 0 but the pc and the stack pointer, which is at the
/// top of the writable page below.
pub(super) fn ready(code: &[u8], look_ahead: LookAhead) -> Machine {
    let mut machine = Machine::new(1 << 20, LOAD, (PAGE / Cpu::ALIGN) as u32, Syscalls::StandIn);
    machine.set_permissions(LOAD..LOAD + PAGE, Permissions::READ | Permissions::EXECUTE);
    machine.set_permissions(LOAD - PAGE..LOAD, Permissions::READ | Permissions::WRITE);
    machine.load(code, 0..code.len() as u64, (LOAD, None), &[0], look_ahead);
    machine.set_registers(&Cpu::initial_block(LOAD, LOAD));
    machine
}
/// Run at none after `prepare` has done something to the machine; the stop and the fields after.
pub(super) fn run(code: &[u8], prepare: impl FnOnce(&Machine)) -> (Stopped, Vec<u128>, Machine) {
    let mut machine = ready(code, LookAhead::None);
    prepare(&machine);
    let (stopped, registers) = machine.run();
    (stopped, fields(&registers).0, machine)
}
pub(super) fn exit_status() {
    let (stopped, _, _) = run(&part::smoke().exit, |_| {});
    assert_eq!(stopped, Stopped::Finished(42));
}
pub(super) fn loop_and_call() {
    let (stopped, _, machine) = run(&part::smoke().loop_and_call, |_| {});
    assert_eq!(stopped, Stopped::Finished(55));
    assert_eq!(machine.count(Reason::StoppedByFlag), 0);
}
/// The stand-in behind the stop hook clears the flag and the thread carries on.
pub(super) fn a_raised_stop_flag_is_cleared_and_the_thread_carries_on() {
    let (stopped, _, machine) = run(&part::smoke().loop_and_call, Machine::raise_stop_flag);
    assert_eq!(stopped, Stopped::Finished(55));
    assert_eq!(machine.count(Reason::StoppedByFlag), 1);
    let warnings: Vec<_> = machine
        .warnings()
        .iter()
        .filter(|w| w.reason == Reason::StoppedByFlag.name())
        .map(|w| w.detail)
        .collect();
    assert_eq!(warnings, [LOAD]);
}
pub(super) fn slow_paths_are_counted_and_warned_once_per_place() {
    let (stopped, _, machine) = run(&part::smoke().unknown_call, |_| {});
    assert_eq!(stopped, Stopped::Finished(3));
    assert_eq!(machine.count(Reason::SystemCallNotImplemented), 2);
    let warnings: Vec<_> = machine
        .warnings()
        .iter()
        .filter(|w| w.reason == Reason::SystemCallNotImplemented.name())
        .map(|w| (w.detail, w.count))
        .collect();
    assert_eq!(warnings, [(UNKNOWN_SYSTEM_CALL, 1)]);
}
/// A flag raised by another thread while a translated loop runs is seen, at every look-ahead: the check must
/// read the flag from memory each time round.
pub(super) fn stop_flag_raised_while_running() {
    let smoke = part::smoke();
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let mut machine = ready(&smoke.long_loop, look_ahead);
        let raise = machine.stop_flag_raiser();
        let raiser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            raise();
        });
        let (stopped, registers) = machine.run();
        raiser.join().unwrap();
        assert_eq!(stopped, Stopped::Finished(0), "{look_ahead:?}");
        assert_eq!(machine.count(Reason::StoppedByFlag), 1, "{look_ahead:?}");
        // It was seen inside the loop, which then carried on to the end.
        let seen_at = machine.warnings()[0].detail - LOAD;
        assert!(
            smoke.long_loop_body.contains(&seen_at),
            "{look_ahead:?}: {seen_at:#x}"
        );
        assert_eq!(fields(&registers).0[smoke.long_loop_counter], 0);
    }
}
/// Checks that a stop raised by another thread with the progress count kept is seen in the loop, as the flag is.
pub(super) fn stop_raised_while_running_with_the_count_kept() {
    let smoke = part::smoke();
    for look_ahead in [LookAhead::None, LookAhead::Full] {
        let mut machine =
            Machine::new(1 << 20, LOAD, (PAGE / Cpu::ALIGN) as u32, Syscalls::StandIn);
        machine.keep_progress_count();
        machine.set_permissions(LOAD..LOAD + PAGE, Permissions::READ | Permissions::EXECUTE);
        machine.set_permissions(LOAD - PAGE..LOAD, Permissions::READ | Permissions::WRITE);
        let code = &smoke.long_loop;
        machine.load(code, 0..code.len() as u64, (LOAD, None), &[0], look_ahead);
        machine.set_registers(&Cpu::initial_block(LOAD, LOAD));
        let raise = machine.stop_flag_raiser();
        let raiser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            raise();
        });
        let (stopped, registers) = machine.run();
        raiser.join().unwrap();
        assert_eq!(stopped, Stopped::Finished(0), "{look_ahead:?}");
        assert_eq!(machine.count(Reason::StoppedByFlag), 1, "{look_ahead:?}");
        let seen_at = machine.warnings()[0].detail - LOAD;
        assert!(
            smoke.long_loop_body.contains(&seen_at),
            "{look_ahead:?}: {seen_at:#x}"
        );
        assert_eq!(fields(&registers).0[smoke.long_loop_counter], 0);
    }
}
/// An instruction of a feature we do not offer raises illegal instruction, at the instruction, and is
/// counted as unsupported; bytes in no form raise it uncounted, as on any CPU.
pub(super) fn instructions_not_offered_are_illegal_and_counted() {
    let smoke = part::smoke();
    for (code, counted) in &smoke.illegal {
        let what = part::disassemble(code);
        let mut program = smoke.mark.clone();
        program.extend(code);
        let (stopped, fields, machine) = run(&program, |_| {});
        assert_eq!(stopped, Stopped::Exit(Exit::IllegalInstruction), "{what}");
        assert_eq!(
            fields[part::PC_FIELD],
            u128::from(LOAD) + smoke.mark.len() as u128,
            "{what}"
        );
        assert_eq!(fields[smoke.marked], 1, "{what}");
        assert_eq!(
            machine.count(Reason::UnsupportedInstructionReached),
            *counted,
            "{what}"
        );
    }
}
