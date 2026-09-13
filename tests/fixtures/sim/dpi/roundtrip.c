#include "svdpi.h"

#include <stdint.h>

int32_t dpi_add(int32_t a, int32_t b) {
    return a + b;
}

void dpi_transform(int32_t a, int32_t *b, int32_t *c) {
    *b = a + 10;
    *c += 20;
}

void dpi_task(int32_t a, int32_t *b) {
    *b = a + 30;
}

svLogic dpi_logic(svLogic value) {
    return value;
}

void dpi_logic_io(svLogic a, svLogic *b, svLogic *c) {
    *b = sv_x;
    *c = a;
}

svLogic dpi_reg(svLogic value) {
    return value;
}

svBit dpi_bit(svBit value) {
    return value;
}

int8_t dpi_byte(int8_t value) {
    return value;
}

uint64_t dpi_u64(uint64_t value) {
    return value;
}

double dpi_real(double value) {
    return value + 0.5;
}

void dpi_real_io(double a, double *b, double *c) {
    *b = a + 1.0;
    *c += 2.0;
}

float dpi_shortreal(float value) {
    return value;
}

void *dpi_handle(void *value) {
    return value;
}

static int dpi_handle_marker;

void dpi_handle_io(void *a, void **b, void **c) {
    *b = a;
    *c = &dpi_handle_marker;
}

const char *dpi_string(const char *value) {
    return value;
}

void dpi_string_io(const char *input_value, char **output_value, char **inout_value) {
    if (!input_value || !inout_value || !*inout_value) {
        *output_value = "dpi-null-error";
        *inout_value = "dpi-null-error";
        return;
    }
    *output_value = "dpi-out";
    *inout_value = "dpi-inout";
}

int32_t pure_add(int32_t a, int32_t b) {
    return a + b;
}

int32_t context_add(int32_t a, int32_t b) {
    return a + b;
}
