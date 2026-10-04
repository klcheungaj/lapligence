// slang_platform.hpp — host-platform services for the C++ wrapper.
//
// Every operating-system and C-library conditional of the wrapper lives in
// slang_platform.cpp; slang_c_api.cpp calls only these neutral functions.
#pragma once

namespace llg::wrapper::platform {

// Return freed heap pages to the operating system where the C library keeps
// them resident after large frees (glibc). A no-op elsewhere. Never throws.
void releaseFreedHeapPages() noexcept;

} // namespace llg::wrapper::platform
