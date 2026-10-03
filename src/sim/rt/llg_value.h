// Compile-time packed-value facade; see value/facade.md.
#ifndef LLG_VALUE_H
#define LLG_VALUE_H
#ifndef LLG_SV4_USE_GMP
#define LLG_SV4_USE_GMP 0
#endif
#define LLG_SV4_SELECTOR_0_TOKEN 1
#define LLG_SV4_SELECTOR_1_TOKEN 1
#define LLG_SV4_SELECTOR_CHECK_I(value) LLG_SV4_SELECTOR_##value##_TOKEN
#define LLG_SV4_SELECTOR_CHECK(value) LLG_SV4_SELECTOR_CHECK_I(value)
#if !LLG_SV4_SELECTOR_CHECK(LLG_SV4_USE_GMP)
#error "LLG_SV4_USE_GMP must be 0 or 1"
#endif
#undef LLG_SV4_SELECTOR_CHECK
#undef LLG_SV4_SELECTOR_CHECK_I
#undef LLG_SV4_SELECTOR_0_TOKEN
#undef LLG_SV4_SELECTOR_1_TOKEN
#ifndef LLG_SV4_GMP_KERNELS
#define LLG_SV4_GMP_KERNELS 0
#endif
#define LLG_KERNEL_SELECTOR_0_TOKEN 1
#define LLG_KERNEL_SELECTOR_1_TOKEN 1
#define LLG_KERNEL_SELECTOR_I(v) LLG_KERNEL_SELECTOR_##v##_TOKEN
#define LLG_KERNEL_SELECTOR(v) LLG_KERNEL_SELECTOR_I(v)
#if !LLG_KERNEL_SELECTOR(LLG_SV4_GMP_KERNELS)
#error "LLG_SV4_GMP_KERNELS must be 0 or 1"
#endif
#if !LLG_SV4_USE_GMP && LLG_SV4_GMP_KERNELS
#error "GMP kernels require the compact value backend"
#endif
#if LLG_SV4_USE_GMP
#define LLG_SV4_GMP_PUBLIC_NAMES
#include "value_gmp/backend.h"
#else
#include "value/backend.h"
#endif
#include "value/bridge.h"
#endif
