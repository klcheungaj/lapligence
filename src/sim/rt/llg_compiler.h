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

// A runtime helper kept out of line so a hot caller does not take on its
// registers and stack frame (a rare path of a per-sample call, for example).
#if defined(__GNUC__) || defined(__clang__)
#define LLG_NOINLINE __attribute__((noinline))
#elif defined(_MSC_VER)
#define LLG_NOINLINE __declspec(noinline)
#else
#define LLG_NOINLINE
#endif

// A runtime function only some designs reach, added to a unit whose hot paths
// are already laid out: out of line and, on ELF targets, in a section of its
// own that the linker puts behind the model's other code, so no other
// function moves. Hot paths are sensitive to where their functions sit
// relative to each other, and a function added among them shifts the ones
// behind it. Other object formats name sections differently; there the
// function is only out of line.
#if (defined(__GNUC__) || defined(__clang__)) && defined(__ELF__)
#define LLG_OUTLYING __attribute__((noinline, section("llg_outlying")))
#else
#define LLG_OUTLYING LLG_NOINLINE
#endif

// A function that runs rarely: optimized for size and, on GCC, left out of
// the unit size its inliner budgets from, so adding one to a unit that sits
// at that budget does not change what is inlined elsewhere in the unit.
#if defined(__GNUC__) || defined(__clang__)
#define LLG_COLD __attribute__((cold))
#else
#define LLG_COLD
#endif

// A runtime function only the generated model calls (never a VPI or DPI
// library): on ELF targets hidden from the dynamic symbol table that models
// export for those libraries, so adding one does not grow that table and move
// the model's data. The runtime is always linked statically into the model.
#if (defined(__GNUC__) || defined(__clang__)) && defined(__ELF__)
#define LLG_INTERNAL __attribute__((visibility("hidden")))
#else
#define LLG_INTERNAL
#endif

// A model definition that may be unreferenced, such as the descriptor of a
// process that only hosts runtime-called callbacks.
#if defined(__GNUC__) || defined(__clang__)
#define LLG_MAYBE_UNUSED __attribute__((unused))
#else
#define LLG_MAYBE_UNUSED
#endif

// Silence GCC's -Wmaybe-uninitialized over a region where a variable is read
// only after the helper that writes it reported success (GCC 13 false
// positive). Initializing the variable instead changes register allocation
// on the hot path, so the generated code is left exactly as it was.
#if defined(__GNUC__) && !defined(__clang__)
#define LLG_MAYBE_UNINIT_BEGIN \
    _Pragma("GCC diagnostic push") _Pragma("GCC diagnostic ignored \"-Wmaybe-uninitialized\"")
#define LLG_MAYBE_UNINIT_END _Pragma("GCC diagnostic pop")
#else
#define LLG_MAYBE_UNINIT_BEGIN
#define LLG_MAYBE_UNINIT_END
#endif

#endif
