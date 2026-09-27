// ── Run-time output placement ───────────────────────────────────────────────
//
// Environment read by every runtime initialization, so one built model can run
// many times with different destinations:
//
// - LLG_SIM_OUT_DIR:   base for relative paths the simulation writes
//                      (waveform, $fopen write/append modes, $writemem*,
//                      LLG_SIM_LOG_FILE). Created when missing. Reads keep
//                      resolving from the current directory.
// - LLG_SIM_WAVE_FILE: waveform file; replaces $dumpfile and `dump.vcd`.
//                      Read by llg_wave.c itself, which stays independent of
//                      this runtime; the directory is created here.
// - LLG_SIM_LOG_FILE:  copy of stdout and stderr (POSIX only).

#if defined(_WIN32)
#include <direct.h>
#include <sys/stat.h>
#else
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

static char* llg_output_dir;  // NULL: relative writes use the CWD

static char* llg_output_strdup(const char* text) {
    size_t len = strlen(text);
    char* copy = (char*)llg_checked_malloc(len + 1u, 1, "output path");
    memcpy(copy, text, len + 1u);
    return copy;
}

static int llg_output_is_separator(char c) {
#if defined(_WIN32)
    return c == '/' || c == '\\';
#else
    return c == '/';
#endif
}

static int llg_output_is_absolute(const char* path) {
    if (llg_output_is_separator(path[0])) return 1;
#if defined(_WIN32)
    return isalpha((unsigned char)path[0]) && path[1] == ':';
#else
    return 0;
#endif
}

// Resolve a path the simulation writes against LLG_SIM_OUT_DIR (absolute paths
// and an unset directory leave it unchanged). The caller frees the result.
static char* llg_output_path(const char* path) {
    if (!path) path = "";
    if (!llg_output_dir || path[0] == '\0' || llg_output_is_absolute(path))
        return llg_output_strdup(path);
    size_t dir_len = strlen(llg_output_dir);
    size_t path_len = strlen(path);
    int separator = dir_len > 0 && !llg_output_is_separator(llg_output_dir[dir_len - 1]);
    char* joined = (char*)llg_checked_malloc(dir_len + (size_t)separator + path_len + 1u, 1,
                                             "output path");
    memcpy(joined, llg_output_dir, dir_len);
    if (separator) joined[dir_len] = '/';
    memcpy(joined + dir_len + (size_t)separator, path, path_len + 1u);
    return joined;
}

static int llg_output_mkdir_one(const char* dir) {
#if defined(_WIN32)
    if (_mkdir(dir) == 0 || errno == EEXIST) return 1;
#else
    if (mkdir(dir, 0777) == 0 || errno == EEXIST) return 1;
#endif
    return 0;
}

static int llg_output_is_dir(const char* dir) {
#if defined(_WIN32)
    struct _stat info;
    return _stat(dir, &info) == 0 && (info.st_mode & _S_IFDIR);
#else
    struct stat info;
    return stat(dir, &info) == 0 && S_ISDIR(info.st_mode);
#endif
}

// `mkdir -p`: create every missing component of `dir`.
static int llg_output_make_dirs(const char* dir) {
    char* path = llg_output_strdup(dir);
    int ok = 1;
    for (char* p = path + 1; *p && ok; ++p) {
        if (!llg_output_is_separator(*p) || llg_output_is_separator(p[-1])) continue;
#if defined(_WIN32)
        if (p[-1] == ':') continue;  // drive root such as `C:\`
#endif
        char saved = *p;
        *p = '\0';
        ok = llg_output_mkdir_one(path);
        *p = saved;
    }
    if (ok) ok = llg_output_mkdir_one(path);
    int saved_errno = errno;
    if (ok && !llg_output_is_dir(path)) {
        ok = 0;
        saved_errno = ENOTDIR;
    }
    free(path);
    errno = saved_errno;
    return ok;
}

// ── Console log (LLG_SIM_LOG_FILE) ──────────────────────────────────────────
//
// stdout and stderr are redirected into pipes read by a forked tee process
// that writes each chunk to the original stream and to the log. A separate
// process (not a thread) keeps output that precedes an abort or crash: the
// tee sees EOF when the simulator dies and still drains the pipes. When both
// streams reach the same file (a terminal, `2>&1`), one shared pipe keeps
// their exact order; separately redirected streams use one pipe each, so only
// the log's stdout/stderr interleaving can differ. Set up once per process;
// later initializations keep the active log.

static int llg_console_log_active;

#if !defined(_WIN32)
static pid_t llg_console_log_pid = -1;
static int llg_console_saved_out = -1;
static int llg_console_saved_err = -1;

static int llg_console_write_all(int fd, const char* data, size_t len) {
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
static void llg_console_tee(int log_fd, int out_read, int err_read, int out, int err) {
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
static void llg_console_log_finish(void) {
    if (llg_console_log_pid < 0) return;
    fflush(stdout);
    fflush(stderr);
    (void)dup2(llg_console_saved_out, STDOUT_FILENO);
    (void)dup2(llg_console_saved_err, STDERR_FILENO);
    close(llg_console_saved_out);
    close(llg_console_saved_err);
    int status;
    while (waitpid(llg_console_log_pid, &status, 0) < 0 && errno == EINTR) {
    }
    llg_console_log_pid = -1;
}

static int llg_console_dup_cloexec(int fd) {
    return fcntl(fd, F_DUPFD_CLOEXEC, 3);
}

static int llg_console_log_start(const char* path) {
    if (llg_console_log_active) return 1;
    char* resolved = llg_output_path(path);
    int log_fd = open(resolved, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0666);
    if (log_fd < 0) {
        fprintf(stderr, "llg: cannot open LLG_SIM_LOG_FILE `%s`: %s\n", resolved,
                strerror(errno));
        free(resolved);
        return 0;
    }
    free(resolved);
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
    llg_console_saved_out = saved_out;
    llg_console_saved_err = saved_err;
    llg_console_log_pid = pid;
    llg_console_log_active = 1;
    atexit(llg_console_log_finish);
    return 1;
}
#else
static int llg_console_log_start(const char* path) {
    (void)path;
    fprintf(stderr, "llg: LLG_SIM_LOG_FILE is not supported on Windows\n");
    return 0;
}
#endif

static const char* llg_output_env(const char* name) {
    const char* value = getenv(name);
    return value && value[0] ? value : NULL;
}

static int configure_output_files(void) {
    free(llg_output_dir);
    llg_output_dir = NULL;
    const char* dir = llg_output_env("LLG_SIM_OUT_DIR");
    if (dir) {
        if (!llg_output_make_dirs(dir)) {
            fprintf(stderr, "llg: cannot create LLG_SIM_OUT_DIR `%s`: %s\n", dir,
                    strerror(errno));
            return 0;
        }
        llg_output_dir = llg_output_strdup(dir);
    }
    const char* log = llg_output_env("LLG_SIM_LOG_FILE");
    return !log || llg_console_log_start(log);
}
