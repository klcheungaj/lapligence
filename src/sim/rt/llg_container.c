/* Private implementation fragments, compiled only through this facade.
 * The embedding in mod.rs uses the same order to emit one flat C source.
 * Do not add these fragments as independent CMake translation units. */
#include "llg_container_prelude.c"
#include "container/value_descriptors.c"
#include "container/dynamic_values.c"
#include "container/queue_values.c"
#include "container/queue_value_mutations.c"
#include "container/dynamic_arrays.c"
#include "container/queues.c"
#include "container/methods.c"
#include "container/queue_references.c"
#include "container/associative_arrays.c"
#include "container/associative_values.c"
#include "container/associative_value_queries.c"
