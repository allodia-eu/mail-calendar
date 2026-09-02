//! **Native faults on Windows**, where they are not signals and no managed handler ever sees them.
//!
//! `Services/CrashLog.cs` covers the .NET side. It cannot cover an access violation inside the
//! cdylib: the CLR tears the process down without raising `AppDomain.UnhandledException`, so the
//! log stops mid-line and Windows Error Reporting holds the only record. This is the handler that
//! puts one line in the file the user can actually hand over.
//!
//! **A vectored handler, not `SetUnhandledExceptionFilter`.** The filter runs only if an exception
//! reaches the top of a thread unhandled, and the CLR installs its own machinery in front of that;
//! a vectored handler is called first, before any frame-based handler, so it sees the fault
//! whatever the runtime goes on to do with it. The cost of being called first is being called for
//! **everything**, which is what the two guards below are for.
//!
//! It never handles anything. Every path returns `EXCEPTION_CONTINUE_SEARCH`, so the CLR still
//! reacts exactly as it would have and Error Reporting still fires: the same "leave the platform's
//! own reporting alone" rule the POSIX side follows by chaining ([`crate::native_fault`]).

use std::{
    ffi::{OsStr, c_void},
    os::windows::ffi::OsStrExt as _,
    ptr,
    sync::{
        Once, OnceLock,
        atomic::{AtomicPtr, AtomicUsize, Ordering},
    },
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, HMODULE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, FILE_APPEND_DATA, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_ALWAYS, WriteFile,
    },
    System::{
        Diagnostics::Debug::{AddVectoredExceptionHandler, EXCEPTION_POINTERS},
        LibraryLoader::{
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            GetModuleFileNameW, GetModuleHandleExW,
        },
        ProcessStatus::{GetModuleInformation, MODULEINFO},
        Threading::GetCurrentProcess,
    },
};

use crate::{
    native_fault::{claim_log, writing_fault_record},
    native_fault_record::{
        ACCESS_VIOLATION, ACCESSING, FATAL, MODULE_CLOSE, SUFFIX, code_name, fault_prefix,
        hex_into, module_prefix,
    },
};

/// "I have not handled this; keep looking." The only value this handler ever returns.
const CONTINUE_SEARCH: i32 = 0;

/// Ask to be called before any handler registered earlier.
const CALL_FIRST: u32 = 1;

/// The log path as a NUL-terminated wide string, prepared at install.
static LOG_PATH: AtomicPtr<u16> = AtomicPtr::new(ptr::null_mut());

/// One finished opening per fault this handler claims, composed at install.
///
/// Composed early for the same reason the POSIX side does it, and here the reason is sharper than
/// "be tidy": one of the codes claimed is `STACK_OVERFLOW`, and a vectored handler for that runs on
/// what is left of an exhausted stack. Allocating a `String` to describe the overflow is a good way
/// to overflow again while describing it, at which point the process dies saying nothing, which is
/// the exact silence this module exists to remove.
static OPENINGS: OnceLock<Vec<(u32, String)>> = OnceLock::new();

/// The opening for `code`, or `None` if this is not a fault worth a record. The lookup **is** the
/// filter: the table is built from `FATAL`, so a code absent from it is declined here.
fn opening_for(code: u32) -> Option<&'static str> {
    OPENINGS
        .get()?
        .iter()
        .find(|(claimed, _)| *claimed == code)
        .map(|(_, opening)| opening.as_str())
}

/// This module's own address range, so a fault elsewhere can be ignored. Zero until located.
///
/// The base is read at the fault for a second reason: subtracting it from the faulting address is
/// what turns a number that dies with the machine into one a symbolizer can resolve.
static MODULE_BASE: AtomicUsize = AtomicUsize::new(0);
static MODULE_END: AtomicUsize = AtomicUsize::new(0);

/// The record's module half (`" (mailcal_bindings.DLL+"`) composed at install, ready for the
/// offset to be written straight after it. The spelling is the loader's own, which is why the
/// extension is upper case here and lower case on disk; Error Reporting spells it the same way.
///
/// `None` if the loader would not name the module. The parenthetical is then dropped whole rather
/// than half-written: an offset resolves against exactly one binary, and one the reader cannot
/// identify is a number with nothing to resolve it against.
static MODULE_OPENING: OnceLock<String> = OnceLock::new();

/// Arms the handler. Once per process, like every other path into this feature.
pub(crate) fn install(log_path: &str) {
    static ARMED: Once = Once::new();
    ARMED.call_once(|| {
        let mut wide: Vec<u16> = OsStr::new(log_path).encode_wide().collect();
        wide.push(0);
        // Leaked on purpose: the handler may read this up to the last instruction the process
        // executes, so it may never be freed.
        LOG_PATH.store(wide.leak().as_mut_ptr(), Ordering::Relaxed);
        let _ = OPENINGS.set(
            FATAL
                .iter()
                .map(|&code| (code, fault_prefix(code_name(code))))
                .collect(),
        );
        // Without our own address range there is no way to tell our fault from .NET's ordinary
        // ones, and a handler that cannot tell them apart is worse than none: so if this fails,
        // nothing is armed at all.
        if !locate_self() {
            return;
        }
        // SAFETY: `handle` has the signature the API requires and outlives the process.
        unsafe {
            AddVectoredExceptionHandler(CALL_FIRST, Some(handle));
        }
    });
}

/// Records where this DLL is loaded, by asking the loader about an address inside it.
fn locate_self() -> bool {
    let mut module = ptr::null_mut();
    // Any address in this module identifies it; a function in this file is the clearest one.
    let anchor = (locate_self as *const ()).cast::<u16>();
    // SAFETY: `anchor` is an address inside this module and both out-params are live locals.
    unsafe {
        // UNCHANGED_REFCOUNT: this only asks where we are. Taking a reference would pin the DLL
        // for the life of the process, which is not this module's business to decide.
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            anchor,
            &raw mut module,
        ) == 0
        {
            return false;
        }
        let mut info: MODULEINFO = std::mem::zeroed();
        let Ok(size) = u32::try_from(size_of::<MODULEINFO>()) else {
            return false;
        };
        if GetModuleInformation(GetCurrentProcess(), module, &raw mut info, size) == 0 {
            return false;
        }
        let base = info.lpBaseOfDll as usize;
        MODULE_BASE.store(base, Ordering::Relaxed);
        MODULE_END.store(base + info.SizeOfImage as usize, Ordering::Relaxed);
    }
    if let Some(name) = module_file_name(module) {
        let _ = MODULE_OPENING.set(module_prefix(&name));
    }
    true
}

/// The file name this module was loaded from, which is what the record names the offset against.
///
/// The **file name alone**, never the path it sits in: that directory is where the user installed
/// the app, and a diagnostic log describes their mail rather than their disk. A symbolizer needs
/// only which binary to open, and support only needs to recognise it.
fn module_file_name(module: HMODULE) -> Option<String> {
    // Sized for the longest path the file system can hold, so the answer is never a truncated
    // one: a truncated path still ends in a plausible file name, which is the worst failure
    // available here. Heap-allocated and dropped immediately, because this runs at install.
    let mut wide = vec![0u16; 32768];
    let capacity = u32::try_from(wide.len()).ok()?;
    // SAFETY: `module` is the handle the loader just gave us, and `wide` is a live buffer of
    // `capacity` `u16`s.
    let written = unsafe { GetModuleFileNameW(module, wide.as_mut_ptr(), capacity) };
    let written = usize::try_from(written).ok()?;
    // Zero is failure, and a full buffer means the path was truncated to fit.
    if written == 0 || written >= wide.len() {
        return None;
    }
    let path = String::from_utf16_lossy(&wide[..written]);
    let name = path.rsplit(['\\', '/']).next().unwrap_or(&path);
    (!name.is_empty()).then(|| name.to_owned())
}

/// Called for every exception raised anywhere in the process, which is why it declines almost all
/// of them.
///
/// Two guards, and the second is the one that makes this safe to ship. .NET raises exceptions as
/// ordinary control flow: a managed `throw` is an exception, and a **caught**
/// `NullReferenceException` is a hardware access violation in JIT-ed code: so a handler that
/// wrote a record for every access violation would fill a user's log with reports of the app
/// working correctly. Only a fault whose faulting instruction lies inside **this DLL** is ours,
/// and nothing inside this DLL raises an exception it expects to survive.
unsafe extern "system" fn handle(pointers: *mut EXCEPTION_POINTERS) -> i32 {
    // SAFETY: the OS hands us a valid `EXCEPTION_POINTERS` for the duration of the call, and every
    // pointer inside it is read only after being checked.
    unsafe {
        if pointers.is_null() {
            return CONTINUE_SEARCH;
        }
        let record = (*pointers).ExceptionRecord;
        if record.is_null() {
            return CONTINUE_SEARCH;
        }
        let code = (*record).ExceptionCode.cast_unsigned();
        let at = (*record).ExceptionAddress as usize;
        let base = MODULE_BASE.load(Ordering::Relaxed);
        if base == 0 || at < base || at >= MODULE_END.load(Ordering::Relaxed) {
            return CONTINUE_SEARCH;
        }
        let Some(opening) = opening_for(code) else {
            return CONTINUE_SEARCH;
        };
        // For an access violation the second parameter is the address the instruction went for,
        // which is the more useful of the two: it separates a null dereference from a pointer that
        // had been somewhere. `NumberParameters` is checked because the record only promises as
        // many as it declares.
        let accessed = if code == ACCESS_VIOLATION && (*record).NumberParameters >= 2 {
            Some((*record).ExceptionInformation[1])
        } else {
            None
        };
        // Taken here because the guard above has just proved `at` is inside this module, and
        // because a fault handler is no place to discover it is about to underflow.
        let offset = at - base;
        if claim_log() {
            // Raised for the length of the record and lowered again after it, so the ordinary log
            // stands down for the write and no longer: this handler declines the fault, so the
            // process may well go on running and go on logging.
            writing_fault_record(true);
            write_record(opening, at, offset, accessed);
            writing_fault_record(false);
        }
    }
    CONTINUE_SEARCH
}

/// Writes the record with the Win32 file API rather than `std::fs`.
///
/// The process is dying and another thread may hold any lock in it, including the ones `std::fs`
/// and the host's own logger take. A raw handle opened here shares nothing.
///
/// **`FILE_APPEND_DATA` is the whole access mask, deliberately.** Win32 appends without a seek
/// only while `FILE_WRITE_DATA` is *withheld*; granted both, the handle honours its file pointer,
/// which `OPEN_ALWAYS` leaves at zero: so adding `GENERIC_WRITE` (which contains
/// `FILE_WRITE_DATA`) does not widen the permission, it silently writes the record over the head
/// of the log. Measured: the record landed at offset 0 and ate the session-start line naming the
/// build that died, which is the one line a support handover needs most.
///
/// The file is opened here rather than kept open from install, for the reason the POSIX side gives:
/// a handle held across a rotation still points at the file that was rotated away, so the record
/// would land in a backup: a file "Share log" does not hand over.
unsafe fn write_record(opening: &str, at: usize, offset: usize, accessed: Option<usize>) {
    let path = LOG_PATH.load(Ordering::Relaxed);
    if path.is_null() {
        return;
    }
    // SAFETY: `path` is the leaked NUL-terminated wide string prepared at install; every other
    // pointer below is a stack buffer owned here.
    unsafe {
        let file: HANDLE = CreateFileW(
            path,
            FILE_APPEND_DATA,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        );
        if file == INVALID_HANDLE_VALUE {
            return;
        }
        write_all(file, opening.as_bytes());
        let mut hex = [0u8; 18];
        write_all(file, hex_into(at, &mut hex));
        if let Some(module) = MODULE_OPENING.get() {
            write_all(file, module.as_bytes());
            write_all(file, hex_into(offset, &mut hex));
            write_all(file, MODULE_CLOSE.as_bytes());
        }
        if let Some(address) = accessed {
            write_all(file, ACCESSING.as_bytes());
            write_all(file, hex_into(address, &mut hex));
        }
        write_all(file, SUFFIX.as_bytes());
        CloseHandle(file);
    }
}

/// `WriteFile` until the buffer is out or the handle refuses it. A record cut in half is the thing
/// this module exists to stop producing.
unsafe fn write_all(file: HANDLE, mut bytes: &[u8]) {
    // SAFETY: `file` is open and `bytes` is a live slice; the loop only advances within it.
    unsafe {
        while !bytes.is_empty() {
            let Ok(len) = u32::try_from(bytes.len()) else {
                return;
            };
            let mut written = 0u32;
            if WriteFile(
                file,
                bytes.as_ptr(),
                len,
                &raw mut written,
                ptr::null_mut::<c_void>().cast(),
            ) == 0
                || written == 0
            {
                return;
            }
            bytes = &bytes[written as usize..];
        }
    }
}

/// The handler driven for real, the Windows form of the POSIX `faulting` test: a child process is
/// armed, made to fault inside itself, and its log read back.
///
/// Nothing short of an actual fault proves any of this. The install can silently fail, the filter
/// can decline everything, and the record can land in the wrong place in the right file, and
/// every unit test in [`crate::native_fault_record`] still passes, because each one only asks what
/// a string looks like. The last of those three is not hypothetical: the access mask granted
/// `FILE_WRITE_DATA` alongside `FILE_APPEND_DATA`, which costs Win32's append-without-a-seek
/// guarantee, and the record overwrote the head of the log instead of being added to it.
///
/// A child rather than a fork, which Windows does not have: the test re-runs itself by name with
/// [`FAULT_CHILD`] set, and that run arms the handler and dereferences null. The code under test is
/// compiled into the test binary rather than the cdylib, so "this module" is that executable and
/// the module-range filter is exercised exactly as it is in the DLL.
#[cfg(test)]
mod faulting {
    use std::{fs, process::Command};

    use super::{ACCESS_VIOLATION, ACCESSING, SUFFIX, code_name, fault_prefix};

    /// Set on the child, holding the log it should write to. Absent in the parent.
    const FAULT_CHILD: &str = "MAILCAL_NATIVE_FAULT_CHILD";

    /// What the log already holds when the fault arrives. The record is *added* to a log in use,
    /// never written over one.
    const EXISTING: &str = "an ordinary line\n";

    /// `EXCEPTION_ACCESS_VIOLATION`, which is the status a process killed by one exits with.
    const ACCESS_VIOLATION_STATUS: i32 = 0xC000_0005_u32.cast_signed();

    /// This test's own path in the binary, which the child is told to run and nothing else.
    ///
    /// Renaming the test without renaming this makes `--exact` match nothing, so the child runs no
    /// test and exits cleanly, and the exit-status assertion below fails saying so, rather than
    /// the child faulting somewhere in the middle of an unrelated suite.
    const THIS_TEST: &str =
        "native_fault_windows::faulting::a_real_fault_writes_its_record_after_the_log_it_found";

    #[test]
    fn a_real_fault_writes_its_record_after_the_log_it_found() {
        if let Ok(log) = std::env::var(FAULT_CHILD) {
            super::install(&log);
            // Volatile so the optimizer cannot decide a null write is unreachable and drop it.
            // SAFETY: deliberately invalid: the fault is what this test exists to raise.
            unsafe { std::ptr::write_volatile(std::ptr::null_mut::<u8>(), 1u8) };
            // Unreachable; a fault that did not happen must not read as a pass.
            std::process::exit(0);
        }

        let dir = std::env::temp_dir().join(format!("mailcal-fault-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("scratch dir");
        let log = dir.join("mailcal.log");
        fs::write(&log, EXISTING).expect("seed the log");

        let status = Command::new(std::env::current_exe().expect("this test binary"))
            .args(["--exact", THIS_TEST])
            .env(FAULT_CHILD, &log)
            .status()
            .expect("run the child");
        let written = fs::read_to_string(&log).expect("read the log back");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(
            status.code(),
            Some(ACCESS_VIOLATION_STATUS),
            "the child did not die of an access violation; log:\n{written}"
        );
        assert!(
            written.starts_with(EXISTING),
            "the record was written OVER the log instead of after it:\n{written}"
        );
        // Through the constants the record is BUILT from, never through a copy of the text. A
        // copy drifts the moment the wording moves, and this test runs on no CI machine (the
        // workspace suite runs on Linux and this file is `cfg(windows)`), so the drift surfaces
        // on whichever developer next runs `cargo test` on Windows. It had: the tail was spelled
        // here with a trailing space where the writer emits `SUFFIX`.
        assert!(
            written.contains(&fault_prefix(code_name(ACCESS_VIOLATION))),
            "no record in:\n{written}"
        );
        assert!(
            written.contains(&format!("{ACCESSING}0x0{SUFFIX}")),
            "the record does not name the address that faulted:\n{written}"
        );

        // The half that survives the machine. A `.exe` here rather than the DLL, for the reason
        // above: the code under test is compiled into this test binary, so this binary is "this
        // module".
        let module = std::env::current_exe()
            .ok()
            .and_then(|exe| {
                exe.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .expect("this test binary's own file name");
        assert!(
            written.contains(&format!(" ({module}+0x")),
            "the record does not say which binary its offset is relative to:\n{written}"
        );

        let (at, offset) = addresses(&written);
        assert!(
            offset < at,
            "{offset:#x} is not an offset into anything loaded at {at:#x}:\n{written}"
        );
        // Whatever is left is the load base, and a real one is page-aligned, which is what says
        // the offset was actually rebased rather than copied from the address beside it. Only a
        // process that has really been loaded somewhere can prove this, so it belongs here and not
        // beside the wording in `crate::native_fault_record`.
        assert_eq!(
            (at - offset) % 0x1000,
            0,
            "{at:#x} - {offset:#x} is not a module base:\n{written}"
        );
    }

    /// The faulting address and the module offset the record carries, as numbers.
    ///
    /// Read back out of the written line rather than compared against what the child computed:
    /// the child is gone, and the log is the only thing a support handover has either.
    fn addresses(record: &str) -> (usize, usize) {
        let number = |after: &str, up_to: char| -> usize {
            let rest = record
                .split(after)
                .nth(1)
                .unwrap_or_else(|| panic!("no {after:?} in:\n{record}"));
            let hex = rest.split(up_to).next().unwrap_or_default();
            usize::from_str_radix(hex.trim_start_matches("0x"), 16)
                .unwrap_or_else(|_| panic!("{hex:?} is not an address in:\n{record}"))
        };
        (number(" at ", ' '), number("+", ')'))
    }
}
