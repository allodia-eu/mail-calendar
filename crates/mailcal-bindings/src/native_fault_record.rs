//! What a native-fault record *says*: the wording, and the numbers in it.
//!
//! Split from the two handlers that write it ([`crate::native_fault`] on POSIX,
//! `native_fault_windows` on Windows) so that all of it is **compiled and unit-tested on every
//! platform**, including the ones whose handler is `cfg`-ed out. A rule exercised on a single
//! platform is how the two records drift into describing the same fault differently, and a
//! `cfg`-ed-out test file reports `running 0 tests ... ok`, which reads exactly like a pass.
//!
//! Everything here is allocation-free at the point of use or prepared before the fault: the POSIX
//! side runs in a signal handler, where allocating is not permitted at all, and the Windows side
//! may be running on the last page of an overflowed stack.
//!
//! Every item below is used by one platform's handler and by this module's own tests on all of
//! them, so `dead_code` here reports only which platform the current build is for: a Windows
//! build has no use for [`signal_prefix`], a POSIX one none for [`is_fatal`]. Warnings are denied
//! crate-wide, so without this a Windows build fails on the POSIX half and vice versa. The tests
//! are what keep the lint's real job covered: nothing here can rot unnoticed, because all of it is
//! asserted on every platform the workspace builds.
#![allow(dead_code, reason = "each platform's handler uses a different half")]

/// The tail of every record, after the addresses.
pub(crate) const SUFFIX: &str = ": the app stopped here ***\n";

/// The opening a POSIX fault writes, up to the faulting address.
///
/// `unhandled` is the word every platform's crash line carries; one string support greps across
/// four clients, and the leading newline puts the record on its own line however far through a
/// line the ordinary log had got when the fault arrived.
pub(crate) fn signal_prefix(signal: &str) -> String {
    format!("\n*** unhandled native fault {signal} at ")
}

/// The opening a Windows fault writes, up to the faulting instruction's address.
///
/// Same shape and the same grep token as [`signal_prefix`]; Windows names a *kind* of fault where
/// POSIX names a signal, because that is what each platform actually reports.
pub(crate) fn fault_prefix(kind: &str) -> String {
    format!("\n*** unhandled native fault, {kind} at ")
}

/// The opening of the module-relative half of a Windows record, up to the offset itself.
///
/// **The absolute address alone does not survive the machine it was written on.** Windows
/// randomizes an image's base at every boot, so the number in a handed-over log names a different
/// byte on the machine that reads it, and the build's debug info: the only thing that can turn a
/// fault back into a function and a line, is indexed by offsets, not by addresses. The
/// parenthetical is the half that travels: `llvm-symbolizer --obj=<the dll> --relative-address
/// <offset>` resolves it months later, against the PDB for the build that died.
///
/// It names the module as well as the offset, because an offset resolves against exactly one
/// binary and a reader has to know which. Windows' own Error Reporting record for the same fault
/// carries these two fields under the names `Faulting module name` and `Fault offset`, so a
/// support handover already speaks them.
///
/// Windows only. POSIX reports `si_addr`, which is the address the faulting instruction *went
/// for* rather than the instruction itself; usually not in any module of ours, and never the
/// number a symbolizer wants.
pub(crate) fn module_prefix(name: &str) -> String {
    format!(" ({name}+")
}

/// Closes it.
pub(crate) const MODULE_CLOSE: &str = ")";

/// What separates the faulting instruction from the address it went for.
///
/// Only an access violation has a second address, and it is the more useful of the two: the
/// instruction says which of our functions died, the operand says whether it was a null
/// dereference or a pointer that had been somewhere.
pub(crate) const ACCESSING: &str = ", accessing ";

// The Windows exception codes worth a record. Plain `u32` rather than the `windows-sys` constants
// so that this module (and its tests) compile everywhere.

/// A read, write or execute of memory the process does not own.
pub(crate) const ACCESS_VIOLATION: u32 = 0xC000_0005;
/// A read that faulted at the pager: a mapped file went away, or the disk did.
pub(crate) const IN_PAGE_ERROR: u32 = 0xC000_0006;
/// Execution reached bytes that are not an instruction.
pub(crate) const ILLEGAL_INSTRUCTION: u32 = 0xC000_001D;
/// A privileged instruction issued from user mode.
pub(crate) const PRIVILEGED_INSTRUCTION: u32 = 0xC000_0096;
/// Integer division by zero.
pub(crate) const INT_DIVIDE_BY_ZERO: u32 = 0xC000_0094;
/// The guard page was hit; there is no stack left to run anything on.
pub(crate) const STACK_OVERFLOW: u32 = 0xC000_00FD;

/// Every Windows exception code that gets a record: the single source both the predicate below
/// and the handler's prepared-openings table are built from, so neither can gain a code the other
/// does not know about.
pub(crate) const FATAL: [u32; 6] = [
    ACCESS_VIOLATION,
    IN_PAGE_ERROR,
    ILLEGAL_INSTRUCTION,
    PRIVILEGED_INSTRUCTION,
    INT_DIVIDE_BY_ZERO,
    STACK_OVERFLOW,
];

/// Whether a Windows exception code is a fault the process cannot continue past.
///
/// The filter matters more here than anywhere else in this feature. A vectored handler is called
/// for **every** exception in the process, and .NET raises them constantly and on purpose; a
/// managed `throw` is exception code `0xE0434352`, and a caught `NullReferenceException` is
/// implemented as a hardware access violation in JIT-ed code. Writing a record for those would
/// fill a user's log with reports of the app working correctly.
pub(crate) fn is_fatal(code: u32) -> bool {
    FATAL.contains(&code)
}

/// How a Windows fault is named in the record.
pub(crate) fn code_name(code: u32) -> &'static str {
    match code {
        ACCESS_VIOLATION => "an access violation",
        IN_PAGE_ERROR => "a page-in error",
        ILLEGAL_INSTRUCTION => "an illegal instruction",
        PRIVILEGED_INSTRUCTION => "a privileged instruction",
        INT_DIVIDE_BY_ZERO => "a division by zero",
        STACK_OVERFLOW => "a stack overflow",
        // Unreachable through `is_fatal`, and deliberately still a sentence: a record that named
        // nothing would be worse than one that admits it does not recognise the code.
        _ => "a fault",
    }
}

/// Renders `value` as `0x`-prefixed lowercase hex into `out`, returning the bytes used.
///
/// The handler needs the faulting address and cannot format one: every formatting path in the
/// standard library allocates. Hand-rolled into a caller-owned buffer, it allocates nothing.
pub(crate) fn hex_into(value: usize, out: &mut [u8; 18]) -> &[u8] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    out[0] = b'0';
    out[1] = b'x';
    if value == 0 {
        out[2] = b'0';
        return &out[..3];
    }
    // Start at the highest set nibble, so the address reads the way every debugger prints it;
    // found by arithmetic rather than by shifting down looking for it, which keeps the whole
    // function branch-light and free of signed arithmetic.
    let mut shift = (usize::BITS - 1 - value.leading_zeros()) / 4 * 4;
    let mut used = 2;
    loop {
        out[used] = DIGITS[(value >> shift) & 0xf];
        used += 1;
        if shift == 0 {
            break;
        }
        shift -= 4;
    }
    &out[..used]
}

#[cfg(test)]
mod tests {
    use super::{
        ACCESS_VIOLATION, ACCESSING, FATAL, ILLEGAL_INSTRUCTION, IN_PAGE_ERROR, INT_DIVIDE_BY_ZERO,
        MODULE_CLOSE, PRIVILEGED_INSTRUCTION, STACK_OVERFLOW, SUFFIX, code_name, fault_prefix,
        hex_into, is_fatal, module_prefix, signal_prefix,
    };

    #[test]
    fn both_platforms_lead_with_the_word_every_client_greps() {
        let posix = signal_prefix("SIGSEGV");
        let windows = fault_prefix("an access violation");

        assert_eq!(posix, "\n*** unhandled native fault SIGSEGV at ");
        assert_eq!(
            windows,
            "\n*** unhandled native fault, an access violation at "
        );
        // The record opens on its own line at both ends: it is appended straight after whatever
        // the ordinary log was part-way through writing when the fault arrived.
        for opening in [&posix, &windows] {
            assert!(
                opening.starts_with("\n*** unhandled native fault"),
                "{opening}"
            );
        }
        assert!(SUFFIX.ends_with('\n'));
    }

    #[test]
    fn an_access_violation_reads_as_two_addresses_and_says_which_is_which() {
        // The instruction says which of our functions died; the operand says whether it went for
        // null or for a pointer that had been somewhere. Run together they would be unreadable,
        // and the wrong one would be taken for the fault site.
        let mut buf = [0u8; 18];
        let line = fault_prefix(code_name(ACCESS_VIOLATION))
            + std::str::from_utf8(hex_into(0x7fff_1234, &mut buf)).expect("ascii")
            + ACCESSING
            + std::str::from_utf8(hex_into(0, &mut buf)).expect("ascii")
            + SUFFIX;

        assert_eq!(
            line,
            concat!(
                "\n*** unhandled native fault, an access violation at 0x7fff1234, accessing 0x0",
                ": the app stopped here ***\n"
            )
        );
    }

    #[test]
    fn a_windows_record_carries_the_one_number_that_outlives_the_machine_it_died_on() {
        // The absolute address is worth nothing to whoever reads the log. Windows randomizes a
        // module's base every boot, so it names a different byte on the machine the support
        // handover lands on, and the build's PDB, which is the only thing that can turn it back
        // into a function, knows nothing but offsets. The parenthetical is the half that travels:
        // `llvm-symbolizer --obj=mailcal_bindings.dll --relative-address 0x10`, months later.
        let mut buf = [0u8; 18];
        let line = fault_prefix(code_name(ACCESS_VIOLATION))
            + std::str::from_utf8(hex_into(0x7ffa_fe32_0010, &mut buf)).expect("ascii")
            + &module_prefix("mailcal_bindings.dll")
            + std::str::from_utf8(hex_into(0x10, &mut buf)).expect("ascii")
            + MODULE_CLOSE
            + ACCESSING
            + std::str::from_utf8(hex_into(0, &mut buf)).expect("ascii")
            + SUFFIX;

        assert_eq!(
            line,
            "\n*** unhandled native fault, an access violation at 0x7ffafe320010 \
             (mailcal_bindings.dll+0x10), accessing 0x0: the app stopped here ***\n"
        );
    }

    #[test]
    fn the_module_half_names_the_binary_and_reads_as_windows_own_report_does() {
        // Error Reporting files the identical fault as "Faulting module name: mailcal_bindings.DLL"
        // and "Fault offset: 0x0000000000000010", so a handover already speaks these two fields.
        // The name is what says *which* build to point a symbolizer at; an offset on its own is a
        // number with nothing to resolve it against.
        let opening = module_prefix("mailcal_bindings.dll");

        assert_eq!(opening, " (mailcal_bindings.dll+");
        assert_eq!(MODULE_CLOSE, ")");
        // Read together with the absolute address, the pair also gives up the load base
        // (`address - offset`), which is what lines the record up with WER's own module list.
    }

    #[test]
    fn an_address_reads_the_way_a_debugger_prints_it() {
        let mut buf = [0u8; 18];

        assert_eq!(hex_into(0, &mut buf), b"0x0");
        assert_eq!(hex_into(1, &mut buf), b"0x1");
        assert_eq!(hex_into(0xdead_beef, &mut buf), b"0xdeadbeef");
    }

    #[test]
    fn the_widest_address_still_fits_the_buffer_the_handler_owns() {
        // That buffer is fixed at 18 bytes because the handler cannot allocate one, and a 64-bit
        // address is `0x` plus 16 nibbles; exactly filling it. An off-by-one here would be a
        // buffer overrun inside a fault handler: unrecoverable, and untraceable afterwards.
        let mut buf = [0u8; 18];

        assert_eq!(
            hex_into(usize::MAX, &mut buf).len(),
            2 + (usize::BITS / 4) as usize
        );
        assert_eq!(hex_into(usize::MAX, &mut buf), b"0xffffffffffffffff");
    }

    #[test]
    fn a_null_dereference_is_distinguishable_from_a_wild_pointer() {
        // The whole reason an address is in the record: `0x0` says a null deref, anything else
        // says the pointer had been somewhere. Without it every fault reads identically.
        let mut buf = [0u8; 18];
        let null = hex_into(0, &mut buf).to_vec();
        let wild = hex_into(0x7fff_0000, &mut buf).to_vec();

        assert_ne!(null, wild);
    }

    #[test]
    fn every_fault_that_kills_a_windows_process_is_claimed_and_named() {
        // The handler builds its openings table from `FATAL` at install, so a code listed there
        // without a name would write "unhandled native fault, a fault at …"; true, and useless.
        for fatal in FATAL {
            assert!(is_fatal(fatal), "{fatal:#x} is not treated as fatal");
            assert_ne!(code_name(fatal), "a fault", "{fatal:#x} has no name");
        }
        for expected in [
            ACCESS_VIOLATION,
            IN_PAGE_ERROR,
            ILLEGAL_INSTRUCTION,
            PRIVILEGED_INSTRUCTION,
            INT_DIVIDE_BY_ZERO,
            STACK_OVERFLOW,
        ] {
            assert!(FATAL.contains(&expected), "{expected:#x} was dropped");
        }
    }

    #[test]
    fn the_exceptions_dotnet_raises_on_purpose_are_not_faults() {
        // The vectored handler sees every exception in the process, and .NET's own are ordinary
        // control flow: `0xE0434352` is a managed `throw`, `0x40010006` is `OutputDebugString`,
        // and `0x406D1388` is the debugger's thread-naming exception. A record for any of these
        // would report the app working correctly, on a log a user hands to support.
        for ordinary in [0xE043_4352, 0x4001_0006, 0x406D_1388, 0x0000_0000] {
            assert!(
                !is_fatal(ordinary),
                "{ordinary:#x} must not read as a crash"
            );
        }
    }

    #[test]
    fn a_breakpoint_is_not_a_crash_either() {
        // `0x80000003` is a breakpoint and `0x80000004` a single step. Both arrive whenever a
        // debugger is attached, which is exactly when a developer is least able to tell a real
        // record from noise.
        assert!(!is_fatal(0x8000_0003));
        assert!(!is_fatal(0x8000_0004));
    }
}
