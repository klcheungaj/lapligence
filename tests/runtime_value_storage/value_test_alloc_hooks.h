#ifndef LLG_VALUE_TEST_ALLOC_HOOKS_H
#define LLG_VALUE_TEST_ALLOC_HOOKS_H
// Force-included into the selected compact value units (CMakeLists.txt) so
// they allocate through the tracked hooks of tracked_value.c or
// storage_runtime.c. <stdlib.h> is included before the renames: renaming on
// the command line would also rename the C library's own declarations, which
// MSVC marks dllimport, and the units would then reference
// __imp_llg_value_test_* symbols that no static archive defines.
#include <stdlib.h>
void* llg_value_test_malloc(size_t bytes);
void* llg_value_test_calloc(size_t count, size_t bytes);
void* llg_value_test_realloc(void* pointer, size_t bytes);
void llg_value_test_free(void* pointer);
#define malloc llg_value_test_malloc
#define calloc llg_value_test_calloc
#define realloc llg_value_test_realloc
#define free llg_value_test_free
#endif
