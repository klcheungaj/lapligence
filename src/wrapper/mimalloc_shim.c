/*
 * mimalloc_shim.c — redirect C allocator symbols to mimalloc via --wrap.
 *
 * The linker flag -Wl,--wrap=malloc renames every unresolved reference to
 * `malloc` in every input object/archive to `__wrap_malloc`, and makes the
 * original definition available as `__real_malloc`.  These shims forward to
 * the mi_* functions so that every C/C++ allocation in the binary — including
 * all of Slang, fmt, and the generated simulator runtime — goes through
 * mimalloc without touching operator new/delete (avoiding the duplicate-symbol
 * conflict with static libstdc++).
 *
 * Forward-declarations of the mi_* functions are inlined here so that this
 * file compiles without needing the mimalloc include path; the definitions are
 * already present in libmimalloc.a which Cargo links in via the `mimalloc`
 * crate dependency.
 */

#include <stddef.h>

/* Forward-declare the mimalloc primitives we need. */
extern void *mi_malloc(size_t size);
extern void *mi_calloc(size_t count, size_t size);
extern void *mi_realloc(void *p, size_t newsize);
extern void  mi_free(void *p);
extern void *mi_aligned_alloc(size_t alignment, size_t size);

void *__wrap_malloc(size_t size)                            { return mi_malloc(size); }
void *__wrap_calloc(size_t n, size_t size)                  { return mi_calloc(n, size); }
void *__wrap_realloc(void *p, size_t size)                  { return mi_realloc(p, size); }
void  __wrap_free(void *p)                                  { mi_free(p); }
void *__wrap_aligned_alloc(size_t alignment, size_t size)   { return mi_aligned_alloc(alignment, size); }
int   __wrap_posix_memalign(void **pp, size_t alignment, size_t size) {
    *pp = mi_aligned_alloc(alignment, size);
    return *pp ? 0 : 12; /* ENOMEM */
}
