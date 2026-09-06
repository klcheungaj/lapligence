#ifndef LLG_STRING_H
#define LLG_STRING_H

#include "llg_value.h"
#include <stddef.h>

/* A string owns its allocation. Expression operations consume their arguments;
 * clone storage before passing it. Zero initialization is the empty string. */
typedef struct { char *data; size_t len; } llg_string_t;
llg_string_t llg_string_bytes(const char *bytes, size_t len);
llg_string_t llg_string_clone(const llg_string_t *value);
void llg_string_destroy(llg_string_t *value);
void llg_string_move(llg_string_t *target, llg_string_t value);
llg_string_t llg_string_concat(llg_string_t a, llg_string_t b);
llg_string_t llg_string_repeat(llg_string_t value, sv4_t count);
llg_string_t llg_string_case(llg_string_t value, int upper);
llg_string_t llg_string_substr(llg_string_t value, sv4_t first, sv4_t last);
llg_string_t llg_string_from_packed(sv4_t value);
sv4_t llg_string_to_packed(llg_string_t value, uint32_t width, int is_signed);
sv4_t llg_string_len(llg_string_t value);
sv4_t llg_string_getc(llg_string_t value, sv4_t index);
void llg_string_putc(llg_string_t *value, sv4_t index, sv4_t character);
sv4_t llg_string_compare(llg_string_t a, llg_string_t b, int ignore_case);
sv4_t llg_string_atoi(llg_string_t value, unsigned base);
void llg_string_itoa(llg_string_t *target, sv4_t value, unsigned base);
void llg_string_print(llg_string_t value);

#endif
