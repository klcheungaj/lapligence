#include "slang_platform.hpp"

// The C library identification macros (__GLIBC__) come from the C library's
// own headers. Without one included first, the test below is always false and
// the glibc page release silently compiles to nothing.
#include <cstdlib>

#if defined(__GLIBC__)
#include <malloc.h>
#endif

namespace llg::wrapper::platform {

void releaseFreedHeapPages() noexcept {
#if defined(__GLIBC__)
  // glibc keeps pages freed inside its main heap resident; malloc_trim
  // returns them. musl's mallocng, the macOS zone allocator and the MSVC CRT
  // heap release large frees on their own.
  malloc_trim(0);
#endif
}

} // namespace llg::wrapper::platform
