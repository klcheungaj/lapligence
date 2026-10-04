// llg_platform.h — operating-system and compiler services for runtime
// implementation files: atomics, host paths and directories, the host stack
// limit and the console log process.
//
// Platform conditionals live only here, in llg_platform_native.h (threads and
// dynamic libraries, which need <windows.h>) and in llg_compiler.h; every
// other runtime source calls the neutral names below. Include this header
// only from runtime .c files, never from a public header or generated model:
// it pulls in POSIX headers. Everything is `static inline` so hot callers
// (the waveform ring) pay no call or indirection, and translation units that
// use only part of the layer compile without unused-function warnings.
// A translation unit that wants POSIX.1-2008 declarations without the GNU
// extensions defines LLG_PLATFORM_POSIX_2008 and includes this header before
// any system header.
#ifndef LLG_PLATFORM_H
#define LLG_PLATFORM_H

#if defined(LLG_PLATFORM_POSIX_2008) && !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif

#include "llg_compiler.h"

#include <ctype.h>
#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(_WIN32)
#include <direct.h>
#include <fcntl.h>
#include <io.h>
#include <sys/stat.h>
#else
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

// ── Atomics ─────────────────────────────────────────────────────────────────
//
// GCC and Clang (including clang-cl and MinGW) use C11 <stdatomic.h>. MSVC's
// C compiler offers <stdatomic.h> only behind /experimental:c11atomics (VS 2022
// 17.5+), and generated models are built with whatever MSVC the user has, so
// MSVC uses its Interlocked intrinsics. Those are full barriers on x64 and
// arm64, at least as strong as every ordering stated below.

#if LLG_COMPILER_MSVC
#include <intrin.h>

typedef volatile __int64 llg_atomic_u64_t;
typedef volatile long llg_atomic_int_t;

// Acquire load. A compare-exchange that stores the value it found never
// changes memory; the cast drops const only for the intrinsic's signature.
static inline uint64_t llg_atomic_u64_load(const llg_atomic_u64_t* p) {
    return (uint64_t)_InterlockedCompareExchange64((llg_atomic_u64_t*)p, 0, 0);
}
// Release store.
static inline void llg_atomic_u64_store(llg_atomic_u64_t* p, uint64_t v) {
    (void)_InterlockedExchange64(p, (__int64)v);
}
// Relaxed (or stronger) add; returns the previous value.
static inline uint64_t llg_atomic_u64_fetch_add(llg_atomic_u64_t* p, uint64_t v) {
    return (uint64_t)_InterlockedExchangeAdd64(p, (__int64)v);
}
// Acquire load.
static inline int llg_atomic_int_load(const llg_atomic_int_t* p) {
    return (int)_InterlockedCompareExchange((llg_atomic_int_t*)p, 0, 0);
}
// Release store.
static inline void llg_atomic_int_store(llg_atomic_int_t* p, int v) {
    (void)_InterlockedExchange(p, (long)v);
}
// Acquire-release compare-exchange; nonzero when *p held `expected`.
static inline int llg_atomic_int_cas(llg_atomic_int_t* p, int expected, int desired) {
    return _InterlockedCompareExchange(p, (long)desired, (long)expected) == (long)expected;
}
// Orders an earlier llg_atomic store before a later llg_atomic load (a
// Dekker-style handshake). Every store above is already a full barrier.
static inline void llg_atomic_store_load_fence(void) {}

#else
#include <stdatomic.h>

typedef _Atomic uint64_t llg_atomic_u64_t;
typedef _Atomic int llg_atomic_int_t;

static inline uint64_t llg_atomic_u64_load(const llg_atomic_u64_t* p) {
    return atomic_load_explicit(p, memory_order_acquire);
}
static inline void llg_atomic_u64_store(llg_atomic_u64_t* p, uint64_t v) {
    atomic_store_explicit(p, v, memory_order_release);
}
static inline uint64_t llg_atomic_u64_fetch_add(llg_atomic_u64_t* p, uint64_t v) {
    return atomic_fetch_add_explicit(p, v, memory_order_relaxed);
}
static inline int llg_atomic_int_load(const llg_atomic_int_t* p) {
    return atomic_load_explicit(p, memory_order_acquire);
}
static inline void llg_atomic_int_store(llg_atomic_int_t* p, int v) {
    atomic_store_explicit(p, v, memory_order_release);
}
static inline int llg_atomic_int_cas(llg_atomic_int_t* p, int expected, int desired) {
    return atomic_compare_exchange_strong_explicit(
        p, &expected, desired, memory_order_acq_rel, memory_order_acquire);
}
static inline void llg_atomic_store_load_fence(void) {
    atomic_thread_fence(memory_order_seq_cst);
}
#endif

// ── Host paths and directories ──────────────────────────────────────────────

// Separator for path lists in environment variables such as LLG_VPI_PLUGIN.
#if defined(_WIN32)
#define LLG_PATH_LIST_SEPARATOR ';'
#else
#define LLG_PATH_LIST_SEPARATOR ':'
#endif

static inline int llg_path_is_separator(char c) {
#if defined(_WIN32)
    return c == '/' || c == '\\';
#else
    return c == '/';
#endif
}

// Rooted (`/x`, `\x`) or drive-qualified (`C:...`) paths are not joined to an
// output directory.
static inline int llg_path_is_absolute(const char* path) {
    if (llg_path_is_separator(path[0])) return 1;
#if defined(_WIN32)
    return isalpha((unsigned char)path[0]) && path[1] == ':';
#else
    return 0;
#endif
}

// True when `c`, just before a separator, ends a drive prefix such as `C:`;
// `mkdir -p` must not try to create that component.
static inline int llg_path_ends_drive_prefix(char c) {
#if defined(_WIN32)
    return c == ':';
#else
    (void)c;
    return 0;
#endif
}

// Create one directory; 0 on success, -1 with errno set otherwise.
static inline int llg_mkdir(const char* dir) {
#if defined(_WIN32)
    return _mkdir(dir);
#else
    return mkdir(dir, 0777);
#endif
}

static inline int llg_path_is_dir(const char* dir) {
#if defined(_WIN32)
    struct _stat info;
    return _stat(dir, &info) == 0 && (info.st_mode & _S_IFDIR);
#else
    struct stat info;
    return stat(dir, &info) == 0 && S_ISDIR(info.st_mode);
#endif
}

// ── Standard streams ────────────────────────────────────────────────────────

// Make stdout and stderr write "\n" unchanged. Windows text-mode streams
// expand it to CRLF; the llg driver (Rust) never does, so this keeps the
// simulator's console output byte-identical to the driver's and to other
// hosts. Files the simulation opens keep their requested text/binary mode.
static inline void llg_stdio_use_lf_newlines(void) {
#if defined(_WIN32)
    fflush(stdout);
    fflush(stderr);
    (void)_setmode(_fileno(stdout), _O_BINARY);
    (void)_setmode(_fileno(stderr), _O_BINARY);
#endif
}

// ── Host stack ──────────────────────────────────────────────────────────────

// Stores the current soft stack limit and returns 1 when the host reports a
// finite one; returns 0 when it is unlimited or unknown (Windows reserves the
// stack at link time instead, see the generated CMake /STACK option).
static inline int llg_host_stack_limit(uint64_t* bytes) {
#if defined(__unix__) || defined(__APPLE__)
    struct rlimit limit;
    if (getrlimit(RLIMIT_STACK, &limit) != 0 || limit.rlim_cur == RLIM_INFINITY) return 0;
    *bytes = (uint64_t)limit.rlim_cur;
    return 1;
#else
    (void)bytes;
    return 0;
#endif
}

// ── Console log process ─────────────────────────────────────────────────────
//
// stdout and stderr are redirected into pipes read by a forked tee process
// that writes each chunk to the original stream and to the log. A separate
// process (not a thread) keeps output that precedes an abort or crash: the
// tee sees EOF when the simulator dies and still drains the pipes. When both
// streams reach the same file (a terminal, `2>&1`), one shared pipe keeps
// their exact order; separately redirected streams use one pipe each, so only
// the log's stdout/stderr interleaving can differ. The tee is joined at exit.
//
// Only the scheduler uses it, and the POSIX side needs POSIX.1-2008
// declarations (O_CLOEXEC, F_DUPFD_CLOEXEC), so a translation unit opts in by
// defining LLG_PLATFORM_CONSOLE_LOG together with a feature-test macro.

#if defined(LLG_PLATFORM_CONSOLE_LOG)

#if !defined(_WIN32)
typedef struct {
    pid_t pid;
    int saved_out;
    int saved_err;
} llg_console_log_state_t;

// One state per process; a function-local static keeps translation units that
// do not use the console log free of unused file-scope variables.
static inline llg_console_log_state_t* llg_console_log_state(void) {
    static llg_console_log_state_t state = {-1, -1, -1};
    return &state;
}

static inline int llg_console_write_all(int fd, const char* data, size_t len) {
    while (len > 0) {
        ssize_t written = write(fd, data, len);
        if (written < 0) {
            if (errno == EINTR) continue;
            return 0;
        }
        data += written;
        len -= (size_t)written;
    }
    return 1;
}

// Child side: only async-signal-safe calls, since the parent may already be
// multithreaded when embedded.
static inline void llg_console_tee(int log_fd, int out_read, int err_read, int out, int err) {
    // Interrupts and a closed console must not cut the log short; the tee
    // exits on EOF once the simulator side has gone.
    signal(SIGINT, SIG_IGN);
    signal(SIGQUIT, SIG_IGN);
    signal(SIGTERM, SIG_IGN);
    signal(SIGHUP, SIG_IGN);
    signal(SIGPIPE, SIG_IGN);
    // err_read < 0: stderr shares the stdout pipe (poll ignores negative fds).
    struct pollfd fds[2] = {{out_read, POLLIN, 0}, {err_read, POLLIN, 0}};
    int targets[2] = {out, err};
    int open_count = err_read < 0 ? 1 : 2;
    static char buffer[65536];
    while (open_count > 0) {
        if (poll(fds, 2, -1) < 0) {
            if (errno == EINTR) continue;
            break;
        }
        for (int i = 0; i < 2; ++i) {
            if (fds[i].fd < 0 || !(fds[i].revents & (POLLIN | POLLHUP | POLLERR))) continue;
            ssize_t count = read(fds[i].fd, buffer, sizeof(buffer));
            if (count < 0 && errno == EINTR) continue;
            if (count <= 0) {
                close(fds[i].fd);
                fds[i].fd = -1;
                --open_count;
                continue;
            }
            if (targets[i] >= 0 && !llg_console_write_all(targets[i], buffer, (size_t)count))
                targets[i] = -1;
            (void)llg_console_write_all(log_fd, buffer, (size_t)count);
        }
    }
    _exit(0);
}

// Restore the original streams and wait until the tee has written everything.
static inline void llg_console_log_finish(void) {
    llg_console_log_state_t* state = llg_console_log_state();
    if (state->pid < 0) return;
    fflush(stdout);
    fflush(stderr);
    (void)dup2(state->saved_out, STDOUT_FILENO);
    (void)dup2(state->saved_err, STDERR_FILENO);
    close(state->saved_out);
    close(state->saved_err);
    int status;
    while (waitpid(state->pid, &status, 0) < 0 && errno == EINTR) {
    }
    state->pid = -1;
}

static inline int llg_console_dup_cloexec(int fd) {
    return fcntl(fd, F_DUPFD_CLOEXEC, 3);
}
#endif

// Start copying stdout and stderr to `path` (already resolved by the caller)
// for the rest of the process. Returns 1 on success; reports and returns 0
// otherwise, including on hosts without a console log implementation.
static inline int llg_console_log_start(const char* path) {
#if !defined(_WIN32)
    int log_fd = open(path, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0666);
    if (log_fd < 0) {
        fprintf(stderr, "llg: cannot open LLG_SIM_LOG_FILE `%s`: %s\n", path, strerror(errno));
        return 0;
    }
    int out_pipe[2] = {-1, -1};
    int err_pipe[2] = {-1, -1};
    fflush(stdout);
    fflush(stderr);
    int saved_out = llg_console_dup_cloexec(STDOUT_FILENO);
    int saved_err = llg_console_dup_cloexec(STDERR_FILENO);
    struct stat out_info;
    struct stat err_info;
    int shared = saved_out >= 0 && saved_err >= 0 && fstat(saved_out, &out_info) == 0 &&
                 fstat(saved_err, &err_info) == 0 && out_info.st_dev == err_info.st_dev &&
                 out_info.st_ino == err_info.st_ino;
    if (saved_out < 0 || saved_err < 0 || pipe(out_pipe) != 0 ||
        (!shared && pipe(err_pipe) != 0)) {
        fprintf(stderr, "llg: cannot set up LLG_SIM_LOG_FILE: %s\n", strerror(errno));
        int fds[] = {log_fd, saved_out, saved_err, out_pipe[0], out_pipe[1],
                     err_pipe[0], err_pipe[1]};
        for (size_t i = 0; i < sizeof(fds) / sizeof(fds[0]); ++i)
            if (fds[i] >= 0) close(fds[i]);
        return 0;
    }
    pid_t pid = fork();
    if (pid < 0) {
        fprintf(stderr, "llg: cannot start LLG_SIM_LOG_FILE writer: %s\n", strerror(errno));
        int fds[] = {log_fd, saved_out, saved_err, out_pipe[0], out_pipe[1],
                     err_pipe[0], err_pipe[1]};
        for (size_t i = 0; i < sizeof(fds) / sizeof(fds[0]); ++i)
            if (fds[i] >= 0) close(fds[i]);
        return 0;
    }
    if (pid == 0) {
        close(out_pipe[1]);
        if (!shared) close(err_pipe[1]);
        llg_console_tee(log_fd, out_pipe[0], shared ? -1 : err_pipe[0], saved_out, saved_err);
    }
    close(log_fd);
    close(out_pipe[0]);
    (void)dup2(out_pipe[1], STDOUT_FILENO);
    if (shared) {
        (void)dup2(out_pipe[1], STDERR_FILENO);
    } else {
        close(err_pipe[0]);
        (void)dup2(err_pipe[1], STDERR_FILENO);
        close(err_pipe[1]);
    }
    close(out_pipe[1]);
    // A pipe makes stdout fully buffered; keep line-at-a-time output when the
    // user is watching a terminal.
    if (isatty(saved_out)) setvbuf(stdout, NULL, _IOLBF, 0);
    llg_console_log_state_t* state = llg_console_log_state();
    state->saved_out = saved_out;
    state->saved_err = saved_err;
    state->pid = pid;
    atexit(llg_console_log_finish);
    return 1;
#else
    (void)path;
    fprintf(stderr, "llg: LLG_SIM_LOG_FILE is not supported on Windows\n");
    return 0;
#endif
}
#endif  // LLG_PLATFORM_CONSOLE_LOG

#endif
