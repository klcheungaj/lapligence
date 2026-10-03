#ifndef LLG_VALUE_BRIDGE_H
#define LLG_VALUE_BRIDGE_H
#ifdef LLG_VALUE_BUILD_CONFIG
#include "../llg_value_build.h"
#endif
#ifndef LLG_VALUE_LINK_GUARD
#if LLG_SV4_USE_GMP
#if LLG_SV4_GMP_KERNELS
#define LLG_VALUE_LINK_GUARD llg_value_v5_b1_k1_unconfigured
#else
#define LLG_VALUE_LINK_GUARD llg_value_v5_b1_k0_portable
#endif
#else
#define LLG_VALUE_LINK_GUARD llg_value_v4_b0_k0_portable
#endif
#endif
#define llg_value_require_abi LLG_VALUE_LINK_GUARD
#ifdef __cplusplus
extern "C" {
#endif
#if LLG_SV4_USE_GMP
#include "../value_gmp/pending.h"
#include "../value_gmp/consumer_bridge.h"
#define llg_ref_read llg_gmp_ref_read
#define llg_ref_view_valid llg_gmp_ref_view_valid
#else
#include "consumer_bridge.h"
#endif
void llg_value_require_abi(void);
int llg_ref_view_valid(const llg_ref_view_t*, const sv4_t*, size_t*);
sv4_t llg_ref_read(const llg_ref_t*);
static inline void llg_sv4_export_vpi32(sv4_t value, void* output,
                                       size_t count, size_t stride) {
    unsigned char* bytes = (unsigned char*)output;
    for (size_t i = 0; i < count;) {
        llg_sv4_vpi_word_t word = llg_sv4_vpi_word(value, i / 2u);
        for (unsigned shift = 0; shift < 64u && i < count; shift += 32u, ++i) {
            uint32_t aval = (uint32_t)(word.aval >> shift);
            uint32_t bval = (uint32_t)(word.bval >> shift);
            memcpy(bytes + i * stride, &aval, sizeof(aval));
            memcpy(bytes + i * stride + sizeof(aval), &bval, sizeof(bval));
        }
    }
}
static inline void llg_sv4_import_vpi32(sv4_t* value, const void* input,
                                       size_t count, size_t stride) {
    const unsigned char* bytes = (const unsigned char*)input;
    size_t available = (llg_sv4_width(*value) + 31u) / 32u;
    if (count > available) count = available;
    for (size_t i = 0; i < count;) {
        size_t word_index = i / 2u;
        llg_sv4_vpi_word_t word = {0, 0};
        if (count - i == 1u) word = llg_sv4_vpi_word(*value, word_index);
        for (unsigned shift = 0; shift < 64u && i < count; shift += 32u, ++i) {
            uint32_t aval, bval;
            memcpy(&aval, bytes + i * stride, sizeof(aval));
            memcpy(&bval, bytes + i * stride + sizeof(aval), sizeof(bval));
            uint64_t mask = (uint64_t)UINT32_MAX << shift;
            word.aval = (word.aval & ~mask) | ((uint64_t)aval << shift);
            word.bval = (word.bval & ~mask) | ((uint64_t)bval << shift);
        }
        llg_sv4_set_vpi_word(value, word_index, word);
    }
}
static inline int llg_sv4_same_vpi_words(sv4_t value,
                                        const llg_sv4_vpi_word_t* words, size_t count) {
    size_t n = llg_sv4_words(value);
    if (n < count) n = count;
    for (size_t i = 0; i < n; ++i) {
        llg_sv4_vpi_word_t a = llg_sv4_vpi_word(value, i);
        llg_sv4_vpi_word_t b = {0, 0};
        if (i < count) b = words[i];
        if (a.aval != b.aval || a.bval != b.bval) return 0;
    }
    return 1;
}
static inline void llg_sv4_export_text(sv4_t value, uint32_t width, char* output) {
    for (size_t i = 0; i < ((size_t)width + 63u) / 64u; ++i) {
        llg_sv4_vpi_word_t word = llg_sv4_vpi_word(value, i);
        unsigned count = width - (uint32_t)(i * 64u);
        if (count > 64u) count = 64u;
        for (unsigned bit = 0; bit < count; ++bit) {
            unsigned a = (unsigned)((word.aval >> bit) & 1u);
            unsigned b = (unsigned)((word.bval >> bit) & 1u);
            output[width - 1u - (uint32_t)(i * 64u) - bit] = b ? (a ? 'x' : 'z') : (a ? '1' : '0');
        }
    }
    output[width] = 0;
}

#ifdef __cplusplus
}
#endif
#endif
