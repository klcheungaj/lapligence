#include "llg_co.h"
#include "probe.h"
#include <string.h>

typedef struct {
    llg_co_frame_t co;
    unsigned char payload[32];
} poison_frame_t;

static llg_co_status_t unused_entry(llg_co_frame_t* co, llg_co_chain_t* ch) {
    (void)co;
    (void)ch;
    return LLG_CO_DONE;
}

static const llg_co_desc_t desc = {
    unused_entry, "poison", sizeof(poison_frame_t), NULL, 0, 0
};

static void check_payload(const poison_frame_t* frame) {
    for (size_t i = 0; i < sizeof(frame->payload); i++) {
#ifdef LLG_CO_DEBUG
        CHECK(frame->payload[i] == LLG_CO_POISON_BYTE);
#else
        CHECK(frame->payload[i] == 0x37);
#endif
    }
}

int main(void) {
    poison_frame_t embedded;
    memset(&embedded, 0x37, sizeof(embedded));
    LLG_CO_DEBUG_POISON_FRAME(&embedded, sizeof(embedded));
    check_payload(&embedded);
    CHECK(embedded.co.state == 0x37373737u);
    CHECK(embedded.co.flags == 0x37373737u);

    LLG_CO_ANCHORED(poison_frame_t) anchored;
    memset(&anchored, 0x37, sizeof(anchored));
    LLG_CO_DEBUG_POISON_FRAME(&anchored.f, sizeof(anchored.f));
    check_payload(&anchored.f);
    CHECK(((const unsigned char*)&anchored.an)[0] == 0x37);

    union {
        unsigned char left[32];
        unsigned char right[32];
    } overlay;
    memset(&overlay, 0x37, sizeof(overlay));
    LLG_CO_DEBUG_POISON(&overlay.left, sizeof(overlay.left));
    for (size_t i = 0; i < sizeof(overlay.right); i++) {
#ifdef LLG_CO_DEBUG
        CHECK(overlay.right[i] == LLG_CO_POISON_BYTE);
#else
        CHECK(overlay.right[i] == 0x37);
#endif
    }

    unsigned loop_exited = 0;
    for (;;) {
        overlay.left[0] = 1;
        LLG_CO_DEBUG_POISON_LOOP_EXIT(overlay.left[0], &overlay, sizeof(overlay));
        loop_exited = 1;
        break;
    }
#ifdef LLG_CO_DEBUG
    CHECK(loop_exited == 0 && overlay.right[0] == LLG_CO_POISON_BYTE);
#else
    CHECK(loop_exited == 1 && overlay.right[0] == 1);
#endif

    sv4_t owner = sv4_from_u64(42, 65, 0);
    CHECK(value_test_live() == 1);
    sv4_destroy(&owner);
    LLG_CO_DEBUG_POISON(&owner, sizeof(owner));
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);

    llg_co_chain_t chain = {0};
    llg_co_anchor_t* slot;
    size_t bytes = sizeof(llg_co_anchor_t) + sizeof(poison_frame_t);
    void* retained = llg_co_arena_push(&chain.arena, 1);
    CHECK(retained != NULL);
    void* seed = llg_co_arena_push(&chain.arena, bytes);
    CHECK(seed != NULL);
    memset(seed, 0x37, bytes);
    llg_co_arena_pop(&chain.arena, seed);
    LLG_CO_ARENA_ENTER(&chain, &desc, slot);
    CHECK((void*)slot == seed);
    check_payload((const poison_frame_t*)LLG_CO_ANCHOR_FRAME(slot));
    llg_co_arena_pop(&chain.arena, slot);
    llg_co_arena_pop(&chain.arena, retained);
    CHECK(chain.arena.head == NULL);
    llg_co_arena_release(&chain.arena);
    puts("frame and overlay poison contract: OK");
    return 0;
}
