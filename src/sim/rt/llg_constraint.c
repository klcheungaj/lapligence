/* Private implementation fragments, compiled only through this facade.
 * The embedding in mod.rs (constraint_sources) uses the same order to emit
 * one flat C source. Do not add these fragments as independent CMake
 * translation units. */
#include "constraints/common.c"
#include "constraints/eval.c"
#include "constraints/aig.c"
#include "constraints/bdd.c"
#include "constraints/solve.c"
