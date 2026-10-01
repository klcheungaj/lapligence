#include "llg_rt.h"

int main(void) {
#ifdef LLG_PROBE_ARENA_ONLY
    llg_co_arena_t arena = {0};
    llg_co_arena_release(&arena);
#else
    llg_rt_init_with_args_and_precision(0, NULL, 1);
    llg_rt_cleanup();
#endif
    return 0;
}
