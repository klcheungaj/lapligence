/* R16: never dereference a caller's unspecified vector union member. */
#include "vpi_user.h"
#include <stdint.h>

static int expected_words(const s_vpi_vecval *words) {
    return words && (uint32_t)words[0].aval == UINT32_C(0x76543210) &&
           (uint32_t)words[0].bval == 0 &&
           (uint32_t)words[1].aval == UINT32_C(0xfedcba98) &&
           (uint32_t)words[1].bval == 0 &&
           (uint32_t)words[2].aval == UINT32_C(0x29) &&
           (uint32_t)words[2].bval == UINT32_C(0x0c);
}

static PLI_INT32 probe(PLI_BYTE8 *data) {
    (void)data;
    vpiHandle signal = vpi_handle_by_name((PLI_BYTE8 *)"tb.payload", NULL);
    s_vpi_value zeroed = {0};
    zeroed.format = vpiVectorVal;
    vpi_get_value(signal, &zeroed);
    if (vpi_chk_error(NULL) || !expected_words(zeroed.value.vector)) return 1;

    /* Non-value queries must not invalidate the returned vector. */
    p_vpi_vecval saved = zeroed.value.vector;
    if (vpi_get(vpiSize, signal) != 70 || !expected_words(saved)) return 1;

    s_vpi_value format_only;
    format_only.format = vpiVectorVal;
    vpi_get_value(signal, &format_only);
    if (vpi_chk_error(NULL) || !expected_words(format_only.value.vector)) return 1;

    s_vpi_value poison = {0};
    poison.format = vpiVectorVal;
    poison.value.vector = (p_vpi_vecval)(uintptr_t)1;
    vpi_get_value(signal, &poison);
    if (vpi_chk_error(NULL) || !expected_words(poison.value.vector)) return 1;

    s_vpi_vecval caller_storage[3] = {{11, 22}, {33, 44}, {55, 66}};
    s_vpi_value supplied = {0};
    supplied.format = vpiVectorVal;
    supplied.value.vector = caller_storage;
    vpi_get_value(signal, &supplied);
    if (vpi_chk_error(NULL) || supplied.value.vector == caller_storage ||
        !expected_words(supplied.value.vector) ||
        caller_storage[0].aval != 11 || caller_storage[0].bval != 22 ||
        caller_storage[1].aval != 33 || caller_storage[1].bval != 44 ||
        caller_storage[2].aval != 55 || caller_storage[2].bval != 66) return 1;
    vpi_printf("vpi vector ownership ok\n");
    return 0;
}

static void register_probe(void) {
    s_vpi_systf_data task = {0};
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8 *)"$vpi_vector_probe";
    task.calltf = probe;
    (void)vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_probe, NULL};
