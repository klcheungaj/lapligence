// llg_compiler.h — compiler-dependent attributes shared by the runtime and
// generated models.
//
// This header and llg_platform.h/llg_platform_native.h are the only runtime
// files that test compiler or operating-system macros; other runtime sources
// and generated code use the neutral names defined here. It includes no
// system header, so public runtime headers and generated models may include it.
// llg_co.h stays self-contained (it ships as a standalone two-file library)
// and keeps its own LLG_CO_* equivalents.
#ifndef LLG_COMPILER_H
#define LLG_COMPILER_H

// MSVC's own C front end; clang-cl defines _MSC_VER too but accepts the
// GCC/Clang dialect.
#if defined(_MSC_VER) && !defined(__clang__)
#define LLG_COMPILER_MSVC 1
#else
#define LLG_COMPILER_MSVC 0
#endif

// Shared instance bodies and batched table rows are called from many sites. Keep
// each one out of line, and on GCC out of interprocedural cloning too, so the
// sharing that saved code size is not undone by the optimizer.
#if defined(__GNUC__) && !defined(__clang__)
#define LLG_MODEL_SHARED __attribute__((noipa))
#elif defined(__clang__)
#define LLG_MODEL_SHARED __attribute__((noinline))
#elif defined(_MSC_VER)
#define LLG_MODEL_SHARED __declspec(noinline)
#else
#define LLG_MODEL_SHARED
#endif

#endif
