#ifndef LLG_STRING_H
#define LLG_STRING_H

#include "llg_value.h"
#include <stddef.h>

/* A string owns its allocation. Expression operations consume their arguments;
 * clone storage before passing it. Zero initialization is the empty string.
 * Persistent model strings optionally retain a typed change callback and
 * dependency marker; owned expression clones leave both fields empty. */
typedef void (*llg_string_notify_fn)(sv4_t *dependency);
typedef struct {
    char *data;
    size_t len;
    llg_string_notify_fn notify;
    sv4_t *dependency;
} llg_string_t;
llg_string_t llg_string_bytes(const char *bytes, size_t len);
llg_string_t llg_string_clone(const llg_string_t *value);
void llg_string_destroy(llg_string_t *value);
/* Transfer an expression owner and clear its slot before a consuming call.
 * This is not a storage write: no dependency notification is performed. */
llg_string_t llg_string_take(llg_string_t *value);
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
/* Consume the string and parse its decimal real prefix; no digits yields zero. */
double llg_string_atoreal(llg_string_t value);
void llg_string_itoa(llg_string_t *target, sv4_t value, unsigned base);
/* Replace target with an owned decimal representation of value. */
void llg_string_realtoa(llg_string_t *target, double value);
void llg_string_print(llg_string_t value);

/* Destination-passing forms of the packed-returning calls above, used by
 * generated code (see value/destinations.h). `X_to(dst, ...)` replaces the
 * initialized owner at dst with exactly the result `X(...)` would return. Packed
 * arguments are borrowed by address; string arguments are consumed through
 * their address and left empty, like the returning forms' by-value strings. */
void llg_string_to_packed_to(sv4_t* dst, llg_string_t* value, uint32_t width, int is_signed);
void llg_string_len_to(sv4_t* dst, llg_string_t* value);
void llg_string_getc_to(sv4_t* dst, llg_string_t* value, const sv4_t* index);
void llg_string_compare_to(sv4_t* dst, llg_string_t* a, llg_string_t* b, int ignore_case);
void llg_string_atoi_to(sv4_t* dst, llg_string_t* value, unsigned base);

/* Replace an expression owner (no change callback) with an owned value. */
void llg_string_replace(llg_string_t *dst, llg_string_t value);
/* Storage writes from an expression owner, with llg_string_move's change
 * notification: move_take consumes *source (left empty), assign copies it. */
void llg_string_move_take(llg_string_t *target, llg_string_t *source);
void llg_string_assign(llg_string_t *target, const llg_string_t *source);
/* String destination forms: `X_to(dst, ...)` replaces the expression owner at
 * dst (destroyed first; it must carry no change callback) with the string
 * `X(...)` would return. Argument conventions match the packed forms above. */
void llg_string_bytes_to(llg_string_t* dst, const char* bytes, size_t len);
void llg_string_clone_to(llg_string_t* dst, const llg_string_t* value);
void llg_string_concat_to(llg_string_t* dst, llg_string_t* a, llg_string_t* b);
void llg_string_repeat_to(llg_string_t* dst, llg_string_t* value, const sv4_t* count);
void llg_string_case_to(llg_string_t* dst, llg_string_t* value, int upper);
void llg_string_substr_to(llg_string_t* dst, llg_string_t* value, const sv4_t* first, const sv4_t* last);
void llg_string_from_packed_to(llg_string_t* dst, const sv4_t* value);

#endif
