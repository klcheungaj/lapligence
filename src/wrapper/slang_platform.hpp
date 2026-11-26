// slang_platform.hpp — host-platform services for the C++ wrapper.
//
// Every operating-system, C-library and compiler conditional of the wrapper
// lives here and in slang_platform.cpp; slang_c_api.cpp uses only these neutral
// names.
#pragma once

// Slang reports a source nested past its limit, and the wrapper a cancelled
// compile or an exceeded budget, by throwing; the C entry points catch both.
// Without exception handling Slang builds its other mode (`#if
// __cpp_exceptions` in slang/util/Util.h), in which each such report aborts
// the process, so the build stops here instead. MSVC and clang-cl need /EHsc
// (build_support/native_cxx.rs).
#if !__cpp_exceptions
#error "the Slang frontend and its wrapper need C++ exception handling"
#endif

// Keeps a function out of its callers: work of some captures only, called
// from a function that every capture runs often, would otherwise change the
// code the compiler makes of the caller and cost the captures that skip it.
#if defined(_MSC_VER) && !defined(__clang__)
#define LLG_NOINLINE __declspec(noinline)
#else
#define LLG_NOINLINE __attribute__((noinline))
#endif

namespace llg::wrapper::platform {

// Return freed heap pages to the operating system where the C library keeps
// them resident after large frees (glibc). A no-op elsewhere. Never throws.
void releaseFreedHeapPages() noexcept;

} // namespace llg::wrapper::platform
