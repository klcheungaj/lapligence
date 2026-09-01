//! Safe, target-specific process-memory primitives.
//!
//! The shared watchdog policy delegates only OS-facing measurements, native
//! limit handles, and emergency I/O to this module.

use std::fmt;
use std::io;

/// Current memory attributed to the complete owning process, including the
/// statically linked Surelog/UHDM frontend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryUsage {
    /// Resident/physical footprint in bytes.
    pub physical_bytes: u64,
    /// Virtual size or committed/pagefile-backed bytes when the platform
    /// exposes a useful value.
    pub virtual_bytes: Option<u64>,
}

/// Native enforcement paired with the portable physical-memory watchdog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnforcementMode {
    WatchdogOnly,
    AddressSpaceAndWatchdog,
    WindowsJobAndWatchdog,
}

impl fmt::Display for EnforcementMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WatchdogOnly => "watchdog-only",
            Self::AddressSpaceAndWatchdog => "address-space+watchdog",
            Self::WindowsJobAndWatchdog => "windows-job+watchdog",
        })
    }
}

/// Error from a process-memory measurement or native limit operation.
#[derive(Debug)]
pub struct MemoryError {
    operation: &'static str,
    source: io::Error,
}

impl MemoryError {
    fn new(operation: &'static str, source: io::Error) -> Self {
        Self { operation, source }
    }
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.source)
    }
}

impl std::error::Error for MemoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn capped_rlimit_as(requested: libc::rlim_t, previous: libc::rlimit) -> libc::rlim_t {
    // A child process must never raise a finite soft limit inherited from its
    // launcher.  RLIM_INFINITY is meaningful for the soft limit only when the
    // hard limit is also considered as the upper bound.
    let inherited_ceiling = if previous.rlim_cur == libc::RLIM_INFINITY {
        previous.rlim_max
    } else {
        previous.rlim_cur
    };
    requested.min(inherited_ceiling)
}

/// Owns any native resource or reversible process limit for as long as the
/// safeguard is installed.
pub struct NativeLimitGuard {
    // The platform value is intentionally retained solely for its Drop
    // implementation (RLIMIT_AS restoration or Job Object handle closure).
    _platform: platform::NativeGuard,
    mode: EnforcementMode,
}

impl NativeLimitGuard {
    pub fn mode(&self) -> EnforcementMode {
        self.mode
    }
}

impl fmt::Debug for NativeLimitGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeLimitGuard")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

/// Measure this process without traversing child processes.  Surelog is
/// linked into this process, so its allocations are included.
pub fn current_usage() -> Result<MemoryUsage, MemoryError> {
    platform::current_usage()
}

/// Install the strongest in-process native limit selected for this platform.
///
/// Windows always attempts a Job Object process-commit limit.  Unix installs
/// `RLIMIT_AS` only when explicitly requested because address space is not the
/// same as physical memory and allocators may reserve large virtual ranges.
pub fn install_native_limit(
    maximum_bytes: u64,
    address_space_limit: bool,
) -> Result<NativeLimitGuard, MemoryError> {
    if maximum_bytes == 0 {
        return Err(MemoryError::new(
            "validate native memory limit",
            io::Error::new(io::ErrorKind::InvalidInput, "memory limit must be nonzero"),
        ));
    }
    let (platform, mode) = platform::install_native_limit(maximum_bytes, address_space_limit)?;
    Ok(NativeLimitGuard {
        _platform: platform,
        mode,
    })
}

/// Write directly to the process' standard-error handle without taking the
/// Rust stderr lock or allocating.  Intended only for the terminal watchdog
/// path.
pub fn write_emergency_stderr(bytes: &[u8]) {
    platform::write_emergency_stderr(bytes);
}

/// Terminate immediately without running destructors or async cleanup.
///
/// This is reserved for the memory watchdog: attempting graceful shutdown
/// after crossing the memory ceiling could allocate further and endanger the
/// host machine.
pub fn terminate_immediately(code: i32) -> ! {
    platform::terminate_immediately(code)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{capped_rlimit_as, EnforcementMode, MemoryError, MemoryUsage};
    use std::io;
    use std::sync::OnceLock;

    static PAGE_SIZE: OnceLock<Result<u64, i32>> = OnceLock::new();

    pub struct NativeGuard {
        previous_limit: Option<libc::rlimit>,
    }

    impl Drop for NativeGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous_limit {
                // SAFETY: `previous` was initialized by a successful
                // `getrlimit(RLIMIT_AS)` call and points to valid storage for
                // the duration of this call.
                let _ = unsafe { libc::setrlimit(libc::RLIMIT_AS, &previous) };
            }
        }
    }

    pub fn current_usage() -> Result<MemoryUsage, MemoryError> {
        let (virtual_pages, resident_pages) = read_statm_pages()?;
        let page_size = page_size()?;
        Ok(MemoryUsage {
            physical_bytes: resident_pages.saturating_mul(page_size),
            virtual_bytes: Some(virtual_pages.saturating_mul(page_size)),
        })
    }

    /// Read the two fields needed from procfs into fixed stack storage.  This
    /// path runs in the watchdog, including when the process is already near
    /// its memory ceiling, so it must not use `read_to_string` or any other
    /// heap-backed convenience API.
    fn read_statm_pages() -> Result<(u64, u64), MemoryError> {
        const PATH: &[u8] = b"/proc/self/statm\0";
        let fd = unsafe {
            // SAFETY: PATH is a NUL-terminated static byte string and the
            // flags request a read-only descriptor for the current process'
            // procfs file.
            libc::open(PATH.as_ptr().cast(), libc::O_RDONLY | libc::O_CLOEXEC)
        };
        if fd < 0 {
            return Err(MemoryError::new(
                "open /proc/self/statm",
                io::Error::last_os_error(),
            ));
        }

        let mut buffer = [0u8; 128];
        let read_result = loop {
            // SAFETY: buffer is valid writable stack storage and its length is
            // within the ssize_t range on every supported Linux target.
            let result = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                // SAFETY: fd was returned by open and is closed exactly once.
                unsafe { libc::close(fd) };
                return Err(MemoryError::new("read /proc/self/statm", error));
            }
            break result as usize;
        };
        // SAFETY: fd was returned by open and is closed exactly once.
        let close_result = unsafe { libc::close(fd) };
        if close_result != 0 {
            return Err(MemoryError::new(
                "close /proc/self/statm",
                io::Error::last_os_error(),
            ));
        }
        parse_statm_pages(&buffer[..read_result])
    }

    fn parse_statm_pages(bytes: &[u8]) -> Result<(u64, u64), MemoryError> {
        let mut values = [0u64; 2];
        let mut count = 0usize;
        let mut value = 0u64;
        let mut in_number = false;
        for &byte in bytes {
            if byte.is_ascii_digit() {
                value = value
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(u64::from(byte - b'0')))
                    .ok_or_else(|| MemoryError::new("parse statm pages", invalid_data()))?;
                in_number = true;
            } else if in_number {
                if count < values.len() {
                    values[count] = value;
                    count += 1;
                }
                value = 0;
                in_number = false;
                if count == values.len() {
                    break;
                }
            }
        }
        if in_number && count < values.len() {
            values[count] = value;
            count += 1;
        }
        if count < values.len() {
            return Err(MemoryError::new("parse statm pages", invalid_data()));
        }
        Ok((values[0], values[1]))
    }

    fn invalid_data() -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, "unexpected statm contents")
    }

    fn page_size() -> Result<u64, MemoryError> {
        match PAGE_SIZE.get_or_init(|| {
            // SAFETY: `_SC_PAGESIZE` is a valid `sysconf` selector and the
            // call has no pointer or lifetime requirements.
            let value = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
            if value > 0 {
                Ok(value as u64)
            } else {
                Err(io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap_or(libc::EINVAL))
            }
        }) {
            Ok(size) => Ok(*size),
            Err(errno) => Err(MemoryError::new(
                "query page size",
                io::Error::from_raw_os_error(*errno),
            )),
        }
    }

    pub fn install_native_limit(
        maximum_bytes: u64,
        address_space_limit: bool,
    ) -> Result<(NativeGuard, EnforcementMode), MemoryError> {
        if !address_space_limit {
            return Ok((
                NativeGuard {
                    previous_limit: None,
                },
                EnforcementMode::WatchdogOnly,
            ));
        }
        install_address_space_limit(maximum_bytes)
    }

    fn install_address_space_limit(
        maximum_bytes: u64,
    ) -> Result<(NativeGuard, EnforcementMode), MemoryError> {
        let mut previous = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        // SAFETY: `previous` points to writable, correctly aligned storage for
        // one `rlimit`; `getrlimit` initializes it on success.
        if unsafe { libc::getrlimit(libc::RLIMIT_AS, previous.as_mut_ptr()) } != 0 {
            return Err(MemoryError::new(
                "read RLIMIT_AS",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: the preceding `getrlimit` call succeeded and initialized
        // every field of `previous`.
        let previous = unsafe { previous.assume_init() };
        let requested = libc::rlim_t::try_from(maximum_bytes).map_err(|_| {
            MemoryError::new(
                "validate RLIMIT_AS",
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "memory limit exceeds the platform rlimit type",
                ),
            )
        })?;
        let current = libc::rlimit {
            // Never raise a limit inherited from the launcher.  A language
            // server may tighten its own ceiling, but it must not weaken a
            // stricter sandbox imposed by a parent process.
            rlim_cur: capped_rlimit_as(requested, previous),
            rlim_max: previous.rlim_max,
        };
        // SAFETY: `current` is fully initialized, its soft limit does not
        // exceed its hard limit, and the pointer remains valid for the call.
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &current) } != 0 {
            return Err(MemoryError::new(
                "set RLIMIT_AS",
                io::Error::last_os_error(),
            ));
        }
        Ok((
            NativeGuard {
                previous_limit: Some(previous),
            },
            EnforcementMode::AddressSpaceAndWatchdog,
        ))
    }

    pub fn write_emergency_stderr(mut bytes: &[u8]) {
        while !bytes.is_empty() {
            // SAFETY: file descriptor 2 is passed by value; `bytes` points to
            // readable storage of the supplied length for the whole call.
            let written =
                unsafe { libc::write(libc::STDERR_FILENO, bytes.as_ptr().cast(), bytes.len()) };
            if written <= 0 {
                break;
            }
            bytes = &bytes[written as usize..];
        }
    }

    pub fn terminate_immediately(code: i32) -> ! {
        // SAFETY: `_exit` accepts every `c_int` and never returns; unlike
        // `exit`, it performs no allocation-prone process cleanup.
        unsafe { libc::_exit(code) }
    }

    #[cfg(test)]
    mod tests {
        use super::parse_statm_pages;

        #[test]
        fn parses_statm_without_heap_backed_text() {
            assert_eq!(
                parse_statm_pages(b"123 45 6 7 8 9 10\n").unwrap(),
                (123, 45)
            );
            assert!(parse_statm_pages(b"123\n").is_err());
            assert!(parse_statm_pages(b"not-statm\n").is_err());
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{capped_rlimit_as, EnforcementMode, MemoryError, MemoryUsage};
    use std::io;

    pub struct NativeGuard {
        previous_limit: Option<libc::rlimit>,
    }

    impl Drop for NativeGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous_limit {
                // SAFETY: `previous` came from a successful `getrlimit` and
                // remains valid for the duration of this call.
                let _ = unsafe { libc::setrlimit(libc::RLIMIT_AS, &previous) };
            }
        }
    }

    pub fn current_usage() -> Result<MemoryUsage, MemoryError> {
        let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v1>::zeroed();
        // SAFETY: `usage` is writable and large enough for RUSAGE_INFO_V1;
        // `getpid` has no preconditions and the kernel initializes the buffer
        // before returning success.
        let result = unsafe {
            libc::proc_pid_rusage(
                libc::getpid(),
                libc::RUSAGE_INFO_V1,
                usage.as_mut_ptr().cast(),
            )
        };
        if result != 0 {
            return Err(MemoryError::new(
                "proc_pid_rusage",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: `proc_pid_rusage` returned success and initialized the V1
        // structure requested above.
        let usage = unsafe { usage.assume_init() };
        Ok(MemoryUsage {
            physical_bytes: usage.ri_phys_footprint.max(usage.ri_resident_size),
            virtual_bytes: None,
        })
    }

    pub fn install_native_limit(
        maximum_bytes: u64,
        address_space_limit: bool,
    ) -> Result<(NativeGuard, EnforcementMode), MemoryError> {
        if !address_space_limit {
            return Ok((
                NativeGuard {
                    previous_limit: None,
                },
                EnforcementMode::WatchdogOnly,
            ));
        }

        let mut previous = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        // SAFETY: `previous` is valid writable storage for one `rlimit`.
        if unsafe { libc::getrlimit(libc::RLIMIT_AS, previous.as_mut_ptr()) } != 0 {
            return Err(MemoryError::new(
                "read RLIMIT_AS",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: successful `getrlimit` initialized `previous`.
        let previous = unsafe { previous.assume_init() };
        let requested = libc::rlim_t::try_from(maximum_bytes).map_err(|_| {
            MemoryError::new(
                "validate RLIMIT_AS",
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "memory limit exceeds the platform rlimit type",
                ),
            )
        })?;
        let current = libc::rlimit {
            // Never raise a limit inherited from the launcher.  A language
            // server may tighten its own ceiling, but it must not weaken a
            // stricter sandbox imposed by a parent process.
            rlim_cur: capped_rlimit_as(requested, previous),
            rlim_max: previous.rlim_max,
        };
        // SAFETY: `current` is initialized and its soft limit is bounded by
        // the existing hard limit.
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &current) } != 0 {
            return Err(MemoryError::new(
                "set RLIMIT_AS",
                io::Error::last_os_error(),
            ));
        }
        Ok((
            NativeGuard {
                previous_limit: Some(previous),
            },
            EnforcementMode::AddressSpaceAndWatchdog,
        ))
    }

    pub fn write_emergency_stderr(mut bytes: &[u8]) {
        while !bytes.is_empty() {
            // SAFETY: `bytes` is valid readable storage and fd 2 is passed by
            // value.  A closed stderr simply makes `write` fail.
            let written =
                unsafe { libc::write(libc::STDERR_FILENO, bytes.as_ptr().cast(), bytes.len()) };
            if written <= 0 {
                break;
            }
            bytes = &bytes[written as usize..];
        }
    }

    pub fn terminate_immediately(code: i32) -> ! {
        // SAFETY: `_exit` accepts every `c_int` and does not return.
        unsafe { libc::_exit(code) }
    }
}

#[cfg(windows)]
mod platform {
    use super::{EnforcementMode, MemoryError, MemoryUsage};
    use std::io;
    use std::mem::{size_of, MaybeUninit};
    use std::ptr;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::WriteFile;
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    };
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};

    pub struct NativeGuard {
        job: HANDLE,
    }

    fn clear_process_memory_limit(job: HANDLE) -> bool {
        // SAFETY: all-zero is a valid value for this Win32 POD structure and
        // yields `LimitFlags = 0`, clearing the limits installed by the guard.
        let limits =
            unsafe { MaybeUninit::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>::zeroed().assume_init() };
        // SAFETY: `job` is a live Job Object handle and `limits` points to a
        // fully initialized structure for the duration of this call.
        let result = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        result != 0
    }

    impl Drop for NativeGuard {
        fn drop(&mut self) {
            if !self.job.is_null() {
                let _ = clear_process_memory_limit(self.job);
                // SAFETY: `job` is a live owned handle returned by
                // `CreateJobObjectW` and is closed exactly once here.
                unsafe { CloseHandle(self.job) };
            }
        }
    }

    pub fn current_usage() -> Result<MemoryUsage, MemoryError> {
        let mut counters = MaybeUninit::<PROCESS_MEMORY_COUNTERS>::zeroed();
        // `GetProcessMemoryInfo` requires the byte size in the structure's
        // first field as well as in its `cb` argument.  Initializing only this
        // field keeps the remainder uninitialized until the call succeeds.
        // SAFETY: `counters` is valid writable storage for the complete POD;
        // writing its first `u32` field does not read any uninitialized data.
        unsafe {
            (*counters.as_mut_ptr()).cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        }
        // SAFETY: `counters` is writable for exactly the size passed;
        // `GetCurrentProcess` returns a valid pseudo-handle for this process.
        let ok = unsafe {
            K32GetProcessMemoryInfo(
                GetCurrentProcess(),
                counters.as_mut_ptr(),
                size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
        };
        if ok == 0 {
            return Err(MemoryError::new(
                "GetProcessMemoryInfo",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: `GetProcessMemoryInfo` returned success and initialized the
        // structure.
        let counters = unsafe { counters.assume_init() };
        Ok(MemoryUsage {
            physical_bytes: counters.WorkingSetSize as u64,
            // `PagefileUsage` is the process commit charge.  It is kept as
            // the optional virtual value; the native cap below also uses the
            // commit-based `ProcessMemoryLimit`, never a working-set limit.
            virtual_bytes: Some(counters.PagefileUsage as u64),
        })
    }

    pub fn install_native_limit(
        maximum_bytes: u64,
        _address_space_limit: bool,
    ) -> Result<(NativeGuard, EnforcementMode), MemoryError> {
        // SAFETY: null security attributes and name request an unnamed Job
        // Object with default security settings.
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job.is_null() {
            return Err(MemoryError::new(
                "CreateJobObjectW",
                io::Error::last_os_error(),
            ));
        }
        let guard = NativeGuard { job };
        let mut limits = MaybeUninit::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>::zeroed();
        // SAFETY: all-zero is a valid initial value for this Win32 POD
        // structure; fields are set before the structure is passed to Win32.
        let mut limits = unsafe { limits.assume_init() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        limits.ProcessMemoryLimit = usize::try_from(maximum_bytes).map_err(|_| {
            MemoryError::new(
                "validate process memory limit",
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "memory limit exceeds the platform addressable size",
                ),
            )
        })?;

        // SAFETY: `guard.job` is valid and owned; `limits` points to a fully
        // initialized structure of the exact size supplied.
        let configured = unsafe {
            SetInformationJobObject(
                guard.job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(MemoryError::new(
                "SetInformationJobObject(process memory limit)",
                io::Error::last_os_error(),
            ));
        }

        // SAFETY: both handles are valid.  Assignment may legitimately fail
        // when a host job disallows nesting; that error is returned so the
        // caller can retain watchdog-only enforcement.
        if unsafe { AssignProcessToJobObject(guard.job, GetCurrentProcess()) } == 0 {
            return Err(MemoryError::new(
                "AssignProcessToJobObject(current process)",
                io::Error::last_os_error(),
            ));
        }
        Ok((guard, EnforcementMode::WindowsJobAndWatchdog))
    }

    pub fn write_emergency_stderr(mut bytes: &[u8]) {
        // SAFETY: `GetStdHandle` has no pointer preconditions.
        let stderr = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
        if stderr.is_null() || stderr == INVALID_HANDLE_VALUE {
            return;
        }
        while !bytes.is_empty() {
            let chunk_len = bytes.len().min(u32::MAX as usize) as u32;
            let mut written = 0u32;
            // SAFETY: `stderr` is a borrowed process handle; the byte slice is
            // readable for `chunk_len`, and `written` is writable for a u32.
            let ok = unsafe {
                WriteFile(
                    stderr,
                    bytes.as_ptr().cast(),
                    chunk_len,
                    &mut written,
                    ptr::null_mut(),
                )
            };
            if ok == 0 || written == 0 {
                break;
            }
            bytes = &bytes[written as usize..];
        }
    }

    pub fn terminate_immediately(code: i32) -> ! {
        // SAFETY: the pseudo-handle identifies the current process and is
        // always valid for `TerminateProcess`.
        unsafe { TerminateProcess(GetCurrentProcess(), code as u32) };
        std::process::abort()
    }

    #[cfg(test)]
    mod tests {
        use super::{clear_process_memory_limit, CloseHandle, CreateJobObjectW};
        use std::io;
        use std::ptr;

        #[test]
        fn clears_limits_on_an_unassigned_job() {
            // SAFETY: null security attributes and name request an unnamed
            // Job Object with default security settings.
            let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
            if job.is_null() {
                eprintln!(
                    "SKIP: CreateJobObjectW unavailable: {}",
                    io::Error::last_os_error()
                );
                return;
            }

            let cleared = clear_process_memory_limit(job);
            let error = io::Error::last_os_error();
            // SAFETY: `job` was returned by `CreateJobObjectW` and is closed
            // exactly once by this test.
            unsafe { CloseHandle(job) };
            assert!(cleared, "SetInformationJobObject failed: {error}");
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    use super::{EnforcementMode, MemoryError, MemoryUsage};
    use std::io;

    pub struct NativeGuard;

    pub fn current_usage() -> Result<MemoryUsage, MemoryError> {
        Err(MemoryError::new(
            "measure process memory",
            io::Error::new(io::ErrorKind::Unsupported, "unsupported operating system"),
        ))
    }

    pub fn install_native_limit(
        _maximum_bytes: u64,
        _address_space_limit: bool,
    ) -> Result<(NativeGuard, EnforcementMode), MemoryError> {
        Ok((NativeGuard, EnforcementMode::WatchdogOnly))
    }

    pub fn write_emergency_stderr(bytes: &[u8]) {
        // There is no supported native measurement backend on this target.
        // Keep the emergency path side-effect free rather than taking a Rust
        // stderr lock or allocating while the process may already be failing.
        let _ = bytes;
    }

    pub fn terminate_immediately(_code: i32) -> ! {
        std::process::abort()
    }
}

#[cfg(test)]
mod tests {
    use super::{current_usage, install_native_limit, EnforcementMode};

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn inherited_soft_limit_is_never_raised() {
        let previous = libc::rlimit {
            rlim_cur: 64,
            rlim_max: 128,
        };
        assert_eq!(super::capped_rlimit_as(96, previous), 64);
        assert_eq!(super::capped_rlimit_as(32, previous), 32);
    }

    #[test]
    fn process_measurement_is_nonzero() {
        let usage = current_usage().expect("current process memory should be measurable");
        assert!(usage.physical_bytes > 0);
    }

    #[cfg(unix)]
    #[test]
    fn disabled_address_space_limit_is_watchdog_only() {
        let guard = install_native_limit(u64::MAX, false).expect("disabled limit is infallible");
        assert_eq!(guard.mode(), EnforcementMode::WatchdogOnly);
    }

    #[test]
    fn zero_native_limit_is_rejected_without_touching_process_limits() {
        assert!(install_native_limit(0, false).is_err());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn address_space_guard_restores_the_previous_limit() {
        fn read_limit() -> libc::rlimit {
            let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
            // SAFETY: `limit` is writable, correctly aligned storage for one
            // rlimit and the kernel initializes it on success.
            let result = unsafe { libc::getrlimit(libc::RLIMIT_AS, limit.as_mut_ptr()) };
            assert_eq!(
                result,
                0,
                "getrlimit failed: {}",
                std::io::Error::last_os_error()
            );
            // SAFETY: the preceding successful getrlimit initialized both
            // fields of the structure.
            unsafe { limit.assume_init() }
        }

        let before = read_limit();
        if before.rlim_cur == 0 {
            return;
        }

        let guard = install_native_limit(u64::MAX, true)
            .expect("an oversized RLIMIT_AS request is feasible");
        assert_eq!(guard.mode(), EnforcementMode::AddressSpaceAndWatchdog);

        let during = read_limit();
        assert!(
            during.rlim_cur <= before.rlim_cur,
            "native limit raised the inherited soft limit: {} -> {}",
            before.rlim_cur,
            during.rlim_cur
        );
        assert_eq!(during.rlim_max, before.rlim_max);

        drop(guard);

        let after = read_limit();
        assert_eq!(after.rlim_cur, before.rlim_cur);
        assert_eq!(after.rlim_max, before.rlim_max);
    }

    #[cfg(windows)]
    #[test]
    fn maximum_job_limit_is_best_effort_and_handle_is_released() {
        // A test process must never install a small limit on itself.  The
        // maximum SIZE_T value is nonrestrictive and still exercises creation,
        // configuration, assignment, and the RAII close path.  Hosts that
        // disallow nested jobs report a normal best-effort error and skip.
        match install_native_limit(u64::MAX, false) {
            Ok(guard) => {
                assert_eq!(guard.mode(), EnforcementMode::WindowsJobAndWatchdog);
                drop(guard);
            }
            Err(error) => eprintln!("SKIP: process job unavailable: {error}"),
        }
    }
}
