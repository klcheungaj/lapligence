#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#include <stdlib.h>
int main(void) {
    sv4_t value = SV4_LITERAL(9, 2, 4, 8, 0);
    sv4_select_plan_t plan = sv4_select_plan_init(8);
    sv4_t result = sv4_select_plan_read(value, &plan);
    llg_ref_t ref = {0};
    ref.base = &value;
    ref.width = 8;
    ref.kind = LLG_REF_WHOLE;
    llg_ref_t* parts[] = {&ref};
    llg_ref_composite_t composite = {1, parts};
    llg_ref_tag_check_t check = {plan, 1, 0, "member"};
    llg_ref_view_t view = {&ref, plan, 1, &check, "test"};
    llg_ref_kind_t kind = LLG_REF_TAGGED_VIEW;
    llg_queue_ref_read_fn reader = NULL;
    llg_queue_ref_write_fn writer = NULL;
    (void)composite;
    (void)kind;
    (void)reader;
    (void)writer;
    if (!llg_ref_view_valid(&view, &value, NULL))
        abort();
    sv4_t read = llg_ref_read(&ref);
    if (!sv4_same(read, value) || !sv4_same(result, value))
        abort();
    sv4_destroy(&read);
    sv4_destroy(&result);
    sv4_destroy(&value);
    return 0;
}
