/* SIM-013 companion for foreign_helpers.sv: a pure import and an import
 * whose only effect is state of this library. */
#include <stdint.h>

static int32_t calls;

int32_t dpi_twice(int32_t value) {
    return value * 2;
}

int32_t dpi_count(int32_t value) {
    ++calls;
    return value;
}
