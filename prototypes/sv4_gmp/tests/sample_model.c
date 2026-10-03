#include "sv4_cell.h"
#include "check.h"

static void run_width(uint32_t width) {
    llg_sv4_cell_t logic = LLG_SV4_CELL_EMPTY, bit = LLG_SV4_CELL_EMPTY;
    llg_sv4_cell_init(&logic, width, 0, 0);
    llg_sv4_cell_init(&bit, width, 0, 1);
    CHECK(sv4_is_unknown(logic.value));
    CHECK(!sv4_is_unknown(bit.value));
    sv4_t a = sv4_from_u64(7, width, 0), b = sv4_from_u64(9, width, 0);
    sv4_t sum = sv4_add(a, b);
    llg_sv4_cell_write(&logic, sum);
    CHECK(sv4_to_u64(logic.value) == 16);
    sv4_t snapshot = llg_sv4_cell_read(&logic);
    sv4_t unknown = sv4_fill(3, width, 0);
    llg_sv4_cell_write(&logic, unknown);
    llg_sv4_cell_write(&bit, unknown);
    CHECK(llg_sv4_state(logic.value, 0) == 3);
    CHECK(!sv4_is_unknown(bit.value) && sv4_to_u64(bit.value) == 0);
    CHECK(sv4_to_u64(snapshot) == 16);
    const sv4_t *drivers[2] = { &a, &unknown };
    sv4_t net = llg_sv4_wire_resolve(drivers, 2, width, 0);
    CHECK(sv4_same(net, a));
    sv4_replace(&unknown, sv4_from_u64(8, width, 0));
    sv4_replace(&net, llg_sv4_wire_resolve(drivers, 2, width, 0));
    CHECK(llg_sv4_state(net, 0) == 2);
    sv4_replace(&a, sv4_fill(3, width, 0));
    sv4_replace(&unknown, sv4_fill(3, width, 0));
    sv4_replace(&net, llg_sv4_wire_resolve(drivers, 2, width, 0));
    CHECK(llg_sv4_state(net, width - 1u) == 3);
    sv4_destroy(&a); sv4_destroy(&b); sv4_destroy(&sum);
    sv4_destroy(&snapshot); sv4_destroy(&unknown); sv4_destroy(&net);
    llg_sv4_cell_destroy(&logic); llg_sv4_cell_destroy(&bit);
}
int main(void) {
    run_width(32); run_width(65); run_width(1024);
    printf("selected backend: %s; sizeof(sv4_t)=%zu; cell=%zu; PASS\n",
           LLG_SV4_BACKEND_NAME, sizeof(sv4_t), sizeof(llg_sv4_cell_t));
    return 0;
}
