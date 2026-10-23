"""Observe actual APU accesses in the existing bounded, source-ROM oracle.

No opcode/control-flow implementation is duplicated here. Optional live NR51
samples enter the original read at its invocation, so two source reads can see
different hardware values. This module is for fixture generation only.
"""
import sys


def run_with_accesses(oracle, rom, bank, context, *, nr51_samples=(), **kwargs):
    reads, writes = [], []
    samples = iter(nr51_samples)
    filename = oracle.run.__code__.co_filename

    def observe(frame, event, value):
        if frame.f_code.co_filename != filename or frame.f_code.co_name != 'read':
            return observe
        address = frame.f_locals['address']
        if 0xff10 <= address <= 0xff3f:
            if event == 'call' and address == 0xff25:
                sample = next(samples, None)
                if sample is not None:
                    # A hardware read sample is not a source write instruction.
                    dict.__setitem__(frame.f_locals['m'], address, sample)
        return observe

    previous = sys.gettrace()
    sys.settrace(observe)
    try:
        trace, registers, memory = oracle.run(
            rom, bank, context, capture=True, strict_memory=True,
            read_events=reads, write_events=writes, **kwargs)
    finally:
        sys.settrace(previous)

    # These bounded routines access ports using LDH a8 (third machine cycle)
    # or LD [HL]/[HLI] (second); the LD a16 form is supported explicitly too.
    # Derive each position from the original opcode, independently of Rust's
    # ApuInstruction constructor. Fail if a new access form reaches this oracle.
    offsets = {0xf0: 8, 0xe0: 8, 0xfa: 12, 0xea: 12,
               0x72: 4, 0x73: 4, 0x77: 4, 0x22: 4}
    accesses = []
    for kind, events in [('Read', reads), ('Write', writes)]:
        for index, pc, address, value in events:
            assert trace[index][0] == pc
            opcode = rom[bank * 0x4000 + pc - 0x4000]
            offset = offsets[opcode]
            assert offset + 4 == trace[index][1]
            accesses.append([index, pc, kind, address, value, offset])
    accesses.sort(key=lambda access: access[0])
    assert len({access[0] for access in accesses}) == len(accesses)
    return trace, registers, memory, accesses
