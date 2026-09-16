/* Static-review counterexample; NOT EXECUTED. */
#include "vpi_user.h"
/* Integration fragment, not a standalone executable.
 * Call with a valid vector object obtained from the simulator.
 */
void review_vector_request(vpiHandle object) {
    s_vpi_value value = {0};
    value.format = vpiVectorVal;
    vpi_get_value(object, &value);
    /* A successful get supplies value.value.vector; no caller allocation. */
    if (value.value.vector == 0)
        vpi_printf("ERROR: simulator did not provide vector storage\n");
}
