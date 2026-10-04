// llg_platform_native.h — threads and dynamic libraries for runtime
// implementation files (see llg_platform.h for the layer's rules).
//
// Kept apart from llg_platform.h because the Windows side needs <windows.h>,
// whose macros must not reach the flat scheduler translation unit; only the
// waveform writer (threads) and the VPI bridge (plugins) include this header.
#ifndef LLG_PLATFORM_NATIVE_H
#define LLG_PLATFORM_NATIVE_H

#include "llg_platform.h"

#if defined(_WIN32)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#else
#include <dlfcn.h>
#include <pthread.h>
#endif

// ── Threads, mutexes and condition variables ────────────────────────────────

#if defined(_WIN32)
typedef HANDLE llg_thread_t;
typedef CRITICAL_SECTION llg_mutex_t;
typedef CONDITION_VARIABLE llg_cond_t;
typedef DWORD llg_thread_id_t;
// Thread entry functions are declared as
// `static LLG_THREAD_RETURN name(void* arg)` and end with
// `return LLG_THREAD_RESULT;`.
#define LLG_THREAD_RETURN DWORD WINAPI
#define LLG_THREAD_RESULT 0
typedef LPTHREAD_START_ROUTINE llg_thread_fn;

static inline void llg_mutex_init(llg_mutex_t* m) { InitializeCriticalSection(m); }
static inline void llg_mutex_destroy(llg_mutex_t* m) { DeleteCriticalSection(m); }
static inline void llg_mutex_lock(llg_mutex_t* m) { EnterCriticalSection(m); }
static inline void llg_mutex_unlock(llg_mutex_t* m) { LeaveCriticalSection(m); }
static inline void llg_cond_init(llg_cond_t* c) { InitializeConditionVariable(c); }
static inline void llg_cond_destroy(llg_cond_t* c) { (void)c; }
static inline void llg_cond_wait(llg_cond_t* c, llg_mutex_t* m) {
    SleepConditionVariableCS(c, m, INFINITE);
}
static inline void llg_cond_signal(llg_cond_t* c) { WakeConditionVariable(c); }
static inline void llg_cond_broadcast(llg_cond_t* c) { WakeAllConditionVariable(c); }
static inline llg_thread_id_t llg_thread_self(void) { return GetCurrentThreadId(); }
static inline int llg_thread_equal(llg_thread_id_t a, llg_thread_id_t b) { return a == b; }

// Starts `fn(arg)` with the default stack size. Returns 0 on success, or
// writes a description of the failure to `error` (NUL-terminated, truncated
// to `error_size`) and returns nonzero.
static inline int llg_thread_start(llg_thread_t* thread, llg_thread_fn fn, void* arg,
                                   char* error, size_t error_size) {
    *thread = CreateThread(NULL, 0, fn, arg, 0, NULL);
    if (*thread) return 0;
    (void)snprintf(error, error_size, "Win32 error %lu", (unsigned long)GetLastError());
    return 1;
}
static inline void llg_thread_join(llg_thread_t thread) {
    (void)WaitForSingleObject(thread, INFINITE);
    CloseHandle(thread);
}
#else
typedef pthread_t llg_thread_t;
typedef pthread_mutex_t llg_mutex_t;
typedef pthread_cond_t llg_cond_t;
typedef pthread_t llg_thread_id_t;
#define LLG_THREAD_RETURN void*
#define LLG_THREAD_RESULT NULL
typedef void* (*llg_thread_fn)(void*);

static inline void llg_mutex_init(llg_mutex_t* m) { (void)pthread_mutex_init(m, NULL); }
static inline void llg_mutex_destroy(llg_mutex_t* m) { (void)pthread_mutex_destroy(m); }
static inline void llg_mutex_lock(llg_mutex_t* m) { (void)pthread_mutex_lock(m); }
static inline void llg_mutex_unlock(llg_mutex_t* m) { (void)pthread_mutex_unlock(m); }
static inline void llg_cond_init(llg_cond_t* c) { (void)pthread_cond_init(c, NULL); }
static inline void llg_cond_destroy(llg_cond_t* c) { (void)pthread_cond_destroy(c); }
static inline void llg_cond_wait(llg_cond_t* c, llg_mutex_t* m) {
    (void)pthread_cond_wait(c, m);
}
static inline void llg_cond_signal(llg_cond_t* c) { (void)pthread_cond_signal(c); }
static inline void llg_cond_broadcast(llg_cond_t* c) { (void)pthread_cond_broadcast(c); }
static inline llg_thread_id_t llg_thread_self(void) { return pthread_self(); }
static inline int llg_thread_equal(llg_thread_id_t a, llg_thread_id_t b) {
    return pthread_equal(a, b);
}

static inline int llg_thread_start(llg_thread_t* thread, llg_thread_fn fn, void* arg,
                                   char* error, size_t error_size) {
    int rc = pthread_create(thread, NULL, fn, arg);
    if (rc == 0) return 0;
    (void)snprintf(error, error_size, "%s", strerror(rc));
    return 1;
}
static inline void llg_thread_join(llg_thread_t thread) { (void)pthread_join(thread, NULL); }
#endif

// ── Dynamic libraries ───────────────────────────────────────────────────────

typedef void* llg_dl_t;

// Loads a library with its symbols resolved now and, on POSIX, made global so
// later plugins can bind to them. NULL on failure; see llg_dl_error.
static inline llg_dl_t llg_dl_open(const char* path) {
#if defined(_WIN32)
    return (llg_dl_t)LoadLibraryA(path);
#else
    return dlopen(path, RTLD_NOW | RTLD_GLOBAL);
#endif
}

// Loader detail for the most recent llg_dl_open failure, or NULL when the
// host keeps none in text form.
static inline const char* llg_dl_error(void) {
#if defined(_WIN32)
    return NULL;
#else
    const char* error = dlerror();
    return error ? error : "unknown loader error";
#endif
}

// The address of exported object `name`, or NULL.
static inline void* llg_dl_symbol(llg_dl_t library, const char* name) {
#if defined(_WIN32)
    // GetProcAddress returns a function-pointer type even for data exports;
    // copy the bits instead of casting between function and object pointers.
    FARPROC proc = GetProcAddress((HMODULE)library, name);
    void* address;
    memcpy(&address, &proc, sizeof(address));
    return address;
#else
    return dlsym(library, name);
#endif
}

static inline void llg_dl_close(llg_dl_t library) {
#if defined(_WIN32)
    FreeLibrary((HMODULE)library);
#else
    dlclose(library);
#endif
}

#endif
