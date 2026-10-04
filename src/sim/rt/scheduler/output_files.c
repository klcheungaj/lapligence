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

static char* llg_output_dir;  // NULL: relative writes use the CWD

static char* llg_output_strdup(const char* text) {
    size_t len = strlen(text);
    char* copy = (char*)llg_checked_malloc(len + 1u, 1, "output path");
    memcpy(copy, text, len + 1u);
    return copy;
}

// Resolve a path the simulation writes against LLG_SIM_OUT_DIR (absolute paths
// and an unset directory leave it unchanged). The caller frees the result.
static char* llg_output_path(const char* path) {
    if (!path) path = "";
    if (!llg_output_dir || path[0] == '\0' || llg_path_is_absolute(path))
        return llg_output_strdup(path);
    size_t dir_len = strlen(llg_output_dir);
    size_t path_len = strlen(path);
    int separator = dir_len > 0 && !llg_path_is_separator(llg_output_dir[dir_len - 1]);
    char* joined = (char*)llg_checked_malloc(dir_len + (size_t)separator + path_len + 1u, 1,
                                             "output path");
    memcpy(joined, llg_output_dir, dir_len);
    if (separator) joined[dir_len] = '/';
    memcpy(joined + dir_len + (size_t)separator, path, path_len + 1u);
    return joined;
}

static int llg_output_mkdir_one(const char* dir) {
    return llg_mkdir(dir) == 0 || errno == EEXIST;
}

// `mkdir -p`: create every missing component of `dir`.
static int llg_output_make_dirs(const char* dir) {
    char* path = llg_output_strdup(dir);
    int ok = 1;
    for (char* p = path + 1; *p && ok; ++p) {
        if (!llg_path_is_separator(*p) || llg_path_is_separator(p[-1])) continue;
        if (llg_path_ends_drive_prefix(p[-1])) continue;  // drive root such as `C:\`
        char saved = *p;
        *p = '\0';
        ok = llg_output_mkdir_one(path);
        *p = saved;
    }
    if (ok) ok = llg_output_mkdir_one(path);
    int saved_errno = errno;
    if (ok && !llg_path_is_dir(path)) {
        ok = 0;
        saved_errno = ENOTDIR;
    }
    free(path);
    errno = saved_errno;
    return ok;
}

// ── Console log (LLG_SIM_LOG_FILE) ──────────────────────────────────────────
//
// A once-per-process tee (see llg_console_log_start in llg_platform.h); later
// initializations keep the active log.

static int llg_console_log_active;

static int llg_output_console_log(const char* path) {
    if (llg_console_log_active) return 1;
    char* resolved = llg_output_path(path);
    int ok = llg_console_log_start(resolved);
    free(resolved);
    if (ok) llg_console_log_active = 1;
    return ok;
}

static const char* llg_output_env(const char* name) {
    const char* value = getenv(name);
    return value && value[0] ? value : NULL;
}

static int configure_output_files(void) {
    llg_stdio_use_lf_newlines();
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
    return !log || llg_output_console_log(log);
}
