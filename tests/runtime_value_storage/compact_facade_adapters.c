#if LLG_ADAPTER_COMPACT
#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#else
#include "llg_value.h"
#endif
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CHECK(c) do { if (!(c)) abort(); } while (0)
_Static_assert(LLG_RESOLVE_WIRE == 0 && LLG_RESOLVE_SUPPLY1 == 6, "net modes");
_Static_assert(LLG_STRENGTH_HIGHZ == 0 && LLG_STRENGTH_SUPPLY == 7, "strength scale");
_Static_assert(LLG_REF_WHOLE == 0 && LLG_REF_TAGGED_VIEW == 9, "reference kinds");
#if LLG_ADAPTER_COMPACT
static const sv4_t literal = SV4_LITERAL(5, 2, 8, 4, 1);
#endif
static sv4_t queue_read(const llg_queue_t* queue, uint64_t identity) {
    (void)queue; return SV4_C(identity, 8);
}
static int queue_write(llg_queue_t* queue, uint64_t identity, sv4_t value) {
    (void)queue; return identity == sv4_to_index(value);
}
int main(int argc, char** argv) {
    if (argc > 1) {
        sv4_t v = SV4_C(0, 65);
        if (!strcmp(argv[1], "unknown")) sv4_replace(&v, SV4_X(65));
        else if (!strcmp(argv[1], "negative")) sv4_replace(&v, sv4_from_i64(-1, 65));
        else if (!strcmp(argv[1], "wide")) llg_sv4_set_state(&v, 64, 1);
        else sv4_replace(&v, SV4_C(LLG_SUPPORTED_WIDTH_LIMIT, 32));
        (void)sv4_checked_width(v); sv4_destroy(&v); return 1;
    }
    sv4_t empty = SV4_EMPTY;
    sv4_t values[] = {SV4_INIT(5, 2, 8, 4, 1), SV4_C(7, 4), SV4_S(15, 4), SV4_X(4), SV4_Z(4)};
    CHECK(llg_sv4_state(values[0], 1) == 2 && llg_sv4_state(values[0], 3) == 3);
    CHECK(sv4_to_index(values[1]) == 7 && sv4_to_i64(values[2]) == -1);
    CHECK(sv4_is_unknown(values[3]) && llg_sv4_has_z(values[4]));
    CHECK(llg_real_to_bool(1.5) && !llg_real_to_bool(0.0));
    CHECK(LLG_MASK(0) == 0 && LLG_MASK(64) == UINT64_MAX);
    for (uint64_t width = 0; width <= 65; ++width) {
        sv4_t v = SV4_C(width, 129); CHECK(sv4_checked_width(v) == width); sv4_destroy(&v);
    }
    sv4_t limit = SV4_C(LLG_SUPPORTED_WIDTH_LIMIT - 1u, 129);
    CHECK(sv4_checked_width(limit) == LLG_SUPPORTED_WIDTH_LIMIT - 1u);
    sv4_select_plan_t plan = {65, 7, 63, 0, 2};
    llg_ref_t ref = {0}; ref.base = &empty; ref.queue_read = queue_read; ref.queue_write = queue_write;
    ref.width = 8; ref.kind = LLG_REF_WHOLE;
    llg_ref_t* parts[] = {&ref}; llg_ref_composite_t composite = {1, parts};
    llg_ref_tag_check_t tag = {plan, 1, 1, "member"};
    llg_ref_view_t view = {&ref, plan, 1, &tag, "native"};
    CHECK(composite.parts[0] == view.parent && view.tag_checks[0].receiver_plan.count == 2);
    sv4_t read = ref.queue_read(NULL, 7); CHECK(ref.queue_write(NULL, 7, read)); sv4_destroy(&read);
#if LLG_ADAPTER_COMPACT
    CHECK(llg_sv4_state(literal, 1) == 2 && llg_sv4_state(literal, 3) == 3);
#endif
    sv4_destroy(&limit); sv4_destroy_array(values, 5); sv4_destroy(&empty);
    puts("facade adapters: passed"); return 0;
}
