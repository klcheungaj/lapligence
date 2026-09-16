/* Static-review counterexample; NOT EXECUTED. */
#include "vpi_user.h"
/* Integration fragment. Invoke at 2 ns for an object in a 1 ns scope. */
void review_time_request(vpiHandle object) {
    s_vpi_time time = {0};
    time.type = vpiScaledRealTime;
    vpi_get_time(object, &time);
    if (time.type != vpiScaledRealTime || time.real != 2.0)
        vpi_printf("ERROR: object-scaled time request was not honored\n");
}
