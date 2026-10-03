/* R05: handwritten collapsed-net configurations, NOT Rust-generated HDL.
 * Exercise the existing resolver/publication ABI used by the new type plan.
 * No coroutine stack switching occurs in either mode. */
#include "llg_rt.c"
#include "probe.h"

static void expect_state(sv4_t value, int state) {
    for (uint32_t bit = 0; bit < value.width; ++bit)
        CHECK(inertial_bit(&value, bit) == state);
}

static void resolver_values(void) {
    static const int kinds[] = {
        LLG_RESOLVE_WIRE, LLG_RESOLVE_WAND, LLG_RESOLVE_WOR,
        LLG_RESOLVE_TRI0, LLG_RESOLVE_TRI1, LLG_RESOLVE_SUPPLY0, LLG_RESOLVE_SUPPLY1
    };
    static const int floats[] = {3, 3, 3, 0, 1, 0, 1};
    static const int conflicts[] = {2, 0, 1, 2, 2, 0, 1};
    static const uint32_t widths[] = {1, 65, 129};
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    for (size_t w = 0; w < sizeof(widths) / sizeof(widths[0]); ++w) {
        for (size_t k = 0; k < sizeof(kinds) / sizeof(kinds[0]); ++k) {
            sv4_t first = sv4_fill(3, widths[w], 0);
            sv4_t second = sv4_fill(3, widths[w], 0);
            sv4_t* const drivers[] = {&first, &second};
            const uint8_t strengths[] = {6, 6};
            llg_net_t net = {0};
            net.width = widths[w];
            net.resolution = (int8_t)kinds[k];
            net.n_drivers = 2;
            net.drivers = drivers;
            net.strength0 = strengths;
            net.strength1 = strengths;
            net.resolved = sv4_fill(3, widths[w], 0);
            llg_net_resolve(&net);
            expect_state(net.resolved, floats[k]);
            if (kinds[k] == LLG_RESOLVE_WAND || kinds[k] == LLG_RESOLVE_WOR) {
                static const unsigned and_table[4][4] = {
                    {0,0,0,0}, {0,1,2,1}, {0,2,2,2}, {0,1,2,3}
                };
                static const unsigned or_table[4][4] = {
                    {0,1,2,0}, {1,1,1,1}, {2,1,2,2}, {0,1,2,3}
                };
                for (int a = 0; a < 4; ++a) {
                    for (int b = 0; b < 4; ++b) {
                        sv4_t av = sv4_fill(a, widths[w], 0);
                        sv4_t bv = sv4_fill(b, widths[w], 0);
                        llg_net_write(&net, 0, av);
                        llg_net_write(&net, 1, bv);
                        sv4_destroy(&av);
                        sv4_destroy(&bv);
                        expect_state(net.resolved, (int)(kinds[k] == LLG_RESOLVE_WAND
                            ? and_table[a][b] : or_table[a][b]));
                        CHECK(value_test_live() == 3 * probe_owner_allocations(widths[w]));
                    }
                }
            }
            for (int round = 0; round < 200; ++round) {
                sv4_t one = sv4_fill(1, widths[w], 0);
                sv4_t zero = sv4_zero(widths[w], 0);
                llg_net_write(&net, 0, one);
                llg_net_write(&net, 1, zero);
                sv4_destroy(&one);
                sv4_destroy(&zero);
                expect_state(net.resolved, conflicts[k]);
                sv4_t floating = sv4_fill(3, widths[w], 0);
                llg_net_write(&net, 0, floating);
                llg_net_write(&net, 1, floating);
                sv4_destroy(&floating);
                expect_state(net.resolved, floats[k]);
                CHECK(value_test_live() == 3 * probe_owner_allocations(widths[w]));
            }
            sv4_destroy(&first);
            sv4_destroy(&second);
            sv4_destroy(&net.resolved);
        }
    }
    /* A selected pull default defeats weak drive, but not strong drive. */
    sv4_t zero = sv4_zero(1, 0);
    const sv4_t* drivers[] = {&zero};
    const uint8_t weak[] = {3}, strong[] = {6};
    sv4_t result = sv4_resolve_strengths(drivers, weak, weak, 1, 1, 0, LLG_RESOLVE_TRI1);
    expect_state(result, 1);
    sv4_replace(&result, sv4_resolve_strengths(drivers, strong, strong, 1, 1, 0, LLG_RESOLVE_TRI1));
    expect_state(result, 0);
    sv4_destroy(&result);
    sv4_destroy(&zero);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("port net-collapse resolver configurations passed");
}

static void delay_publication(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t first = sv4_fill(3, 1, 0), second = sv4_fill(3, 1, 0);
    sv4_t* const drivers[] = {&first, &second};
    const uint8_t strengths[] = {6, 6};
    llg_net_t net = {0};
    net.resolved = sv4_fill(3, 1, 0);
    net.width = 1;
    net.resolution = LLG_RESOLVE_WAND;
    net.n_drivers = 2;
    net.drivers = drivers;
    net.strength0 = strengths;
    net.strength1 = strengths;
    net.propagation_enabled = 1;
    net.propagation_rise = 3;
    net.propagation_fall = 5;
    net.propagation_turn_off = 2;
    sv4_t storage = sv4_from_u64(2, 2, 0);
    sv4_t published = sv4_fill(3, 2, 0);
    const llg_net_alias_part_t part = {&net, 0, 0, 0, 1};
    llg_net_alias_t alias = {0};
    alias.storage = &storage;
    alias.width = 2;
    alias.parts = &part;
    alias.n_parts = 1;
    alias.publication_target = &published;
    llg_net_alias_bind(&alias);
    CHECK(inertial_bit(&published, 0) == 3 && inertial_bit(&published, 1) == 1);
    g.now = 1;
    sv4_t value = sv4_from_u64(1, 1, 0);
    llg_net_write(&net, 0, value);
    sv4_destroy(&value);
    g.now = 2; commit_inertial(LLG_REGION_ACTIVE);
    expect_state(net.resolved, 3);
    CHECK(inertial_bit(&published, 0) == 3);
    g.now = 4; commit_inertial(LLG_REGION_ACTIVE);
    expect_state(net.resolved, 1);
    CHECK(sv4_to_u64(published) == 3 && sv4_to_u64(alias.visible) == 3);
    g.now = 5;
    value = sv4_zero(1, 0);
    llg_net_write(&net, 1, value);
    sv4_destroy(&value);
    g.now = 9; commit_inertial(LLG_REGION_ACTIVE);
    expect_state(net.resolved, 1);
    g.now = 10; commit_inertial(LLG_REGION_ACTIVE);
    expect_state(net.resolved, 0);
    CHECK(sv4_to_u64(published) == 2 && sv4_to_u64(alias.visible) == 2);
    g.now = 11;
    value = sv4_fill(3, 1, 0);
    llg_net_write(&net, 0, value);
    llg_net_write(&net, 1, value);
    sv4_destroy(&value);
    g.now = 12; commit_inertial(LLG_REGION_ACTIVE);
    expect_state(net.resolved, 0);
    g.now = 13; commit_inertial(LLG_REGION_ACTIVE);
    expect_state(net.resolved, 3);
    CHECK(inertial_bit(&published, 0) == 3 && inertial_bit(&published, 1) == 1);
    CHECK(sv4_same(published, alias.visible));
    llg_rt_cleanup(); /* The borrowed net/handle/alias objects are still alive. */
    CHECK(net.propagation == NULL);
    llg_net_alias_clear(&net);
    sv4_destroy(&alias.visible);
    sv4_destroy(&published);
    sv4_destroy(&storage);
    sv4_destroy(&first);
    sv4_destroy(&second);
    sv4_destroy(&net.resolved);
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("port net-collapse delayed alias/array publication passed");
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    if (!strcmp(argv[1], "values")) resolver_values();
    else if (!strcmp(argv[1], "publication")) delay_publication();
    else return 2;
    return 0;
}
