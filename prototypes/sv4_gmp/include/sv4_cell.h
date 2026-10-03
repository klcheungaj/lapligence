#ifndef LLG_PROTOTYPE_SV4_CELL_H
#define LLG_PROTOTYPE_SV4_CELL_H
#include "sv4.h"
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>

/* Illustrative variable cell, not a registered scheduler cell. Keep its address
 * stable while borrowed. Four-state values currently known are still four-state. */
typedef struct { sv4_t value; unsigned two_state; } llg_sv4_cell_t;
#define LLG_SV4_CELL_EMPTY { SV4_EMPTY, 0 }
static inline void llg_sv4_cell_init(llg_sv4_cell_t *cell, uint32_t width,
                                    int8_t sign, unsigned two_state) {
    cell->two_state = two_state != 0;
    sv4_replace(&cell->value, sv4_fill(cell->two_state ? 0 : 2, width, sign));
}
static inline void llg_sv4_cell_write(llg_sv4_cell_t *cell, sv4_t source) {
    sv4_t converted = sv4_cast(source, llg_sv4_width(cell->value),
                              llg_sv4_signed(cell->value));
    if (cell->two_state) sv4_replace(&converted, sv4_to_two_state(converted));
    sv4_move(&cell->value, &converted);
}
static inline sv4_t llg_sv4_cell_read(const llg_sv4_cell_t *cell) {
    return sv4_clone(&cell->value);
}
static inline void llg_sv4_cell_destroy(llg_sv4_cell_t *cell) {
    sv4_destroy(&cell->value);
    cell->two_state = 0;
}
static inline sv4_t llg_sv4_wire_resolve(const sv4_t *const *drivers,
                                        size_t count, uint32_t width, int8_t sign) {
    if ((count && !drivers) || count > INT_MAX) {
        fputs("sv4 prototype: invalid driver table\n", stderr); abort();
    }
    for (size_t i = 0; i < count; ++i) {
        if (drivers[i] && llg_sv4_width(*drivers[i]) != width) {
            fputs("sv4 prototype: driver width mismatch\n", stderr); abort();
        }
    }
#if LLG_SV4_USE_GMP
    return gmp4_resolve_wire(drivers, count, width, sign);
#else
    return sv4_resolve(drivers, (int)count, width, sign, LLG_RESOLVE_WIRE);
#endif
}
#endif
