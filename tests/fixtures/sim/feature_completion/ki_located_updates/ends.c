#include <stdint.h>
extern int32_t from_c(int32_t k);
int32_t through_c(int32_t k) { return from_c(k) * 10; }
