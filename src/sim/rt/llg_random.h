// llg_random.h — Verilog-2001 probabilistic distribution functions.
//
// The implementations in llg_random.c are the Annex N algorithms from IEEE
// Std 1364-2001 / IEEE Std 1800-2009 Annex N.  The seed is an in/out 32-bit
// signed value.  Keeping this module independent from llg_rt.c makes the
// algorithm available to standalone runtime tests as well as generated
// models.
#ifndef LLG_RANDOM_H
#define LLG_RANDOM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Generate the next legacy random value with a non-null writable seed.
int32_t llg_random_next(int32_t *seed);

// Generate the next legacy random value using the model's implicit stream.
// Defined by the scheduler runtime (llg_rt.c), since a random call is a check
// point for waiters (T1-D11); the seed starts at LLG_RANDOM_DEFAULT_SEED.
int32_t llg_random_default(void);
enum { LLG_RANDOM_DEFAULT_SEED = 259341593 };

int32_t llg_dist_uniform(int32_t *seed, int32_t start, int32_t end);
int32_t llg_dist_normal(int32_t *seed, int32_t mean, int32_t deviation);
int32_t llg_dist_exponential(int32_t *seed, int32_t mean);
int32_t llg_dist_poisson(int32_t *seed, int32_t mean);
int32_t llg_dist_chi_square(int32_t *seed, int32_t degree_of_freedom);
int32_t llg_dist_t(int32_t *seed, int32_t degree_of_freedom);
int32_t llg_dist_erlang(int32_t *seed, int32_t k_stage, int32_t mean);

#ifdef __cplusplus
}
#endif

#endif
