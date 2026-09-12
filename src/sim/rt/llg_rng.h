#ifndef LLG_RNG_H
#define LLG_RNG_H

#include <stdint.h>

#include "llg_string.h"

#ifdef __cplusplus
extern "C" {
#endif

/*
 * One deterministic pseudo-random stream.  `state` advances on every draw;
 * `increment` identifies the stream and is deliberately immutable between
 * explicit seeding operations.  `child_count` is part of the hierarchy, not
 * the draw state, so random calls in a parent cannot perturb a later sibling.
 */
typedef struct {
    uint64_t state;
    uint64_t increment;
    uint64_t child_count;
} llg_rng_state_t;

#define LLG_RNG_DEFAULT_SEED UINT64_C(0x4d595df4d0f33173)

void llg_rng_state_seed(llg_rng_state_t* state, uint64_t seed);
void llg_rng_state_derive(llg_rng_state_t* child,
                          const llg_rng_state_t* parent,
                          uint64_t ordinal);
void llg_rng_state_child(llg_rng_state_t* parent, llg_rng_state_t* child);
uint32_t llg_rng_state_next(llg_rng_state_t* state);
uint32_t llg_rng_state_uniform(llg_rng_state_t* state,
                               uint32_t first, uint32_t second);

/* State strings are opaque to HDL.  The versioned text is intentionally
 * stable across model rebuilds and can be passed to set_randstate later. */
llg_string_t llg_rng_state_get(const llg_rng_state_t* state);
int llg_rng_state_set(llg_rng_state_t* state, const llg_string_t* encoded);

#ifdef __cplusplus
}
#endif

#endif
