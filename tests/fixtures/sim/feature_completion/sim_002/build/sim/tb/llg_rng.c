#include "llg_rng.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void rng_fail(const char* message) {
    fprintf(stderr, "llg: random runtime: %s\n", message);
    abort();
}

static uint64_t splitmix64(uint64_t value) {
    value += UINT64_C(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)) * UINT64_C(0x94d049bb133111eb);
    return value ^ (value >> 31);
}

void llg_rng_state_seed(llg_rng_state_t* state, uint64_t seed) {
    if (!state) rng_fail("seeding a null stream");
    uint64_t stream = splitmix64(seed ^ UINT64_C(0x243f6a8885a308d3));
    state->increment = stream | UINT64_C(1);
    state->state = 0;
    state->child_count = 0;
    /* PCG's initialization sequence prevents nearby seeds from sharing a
     * short prefix while retaining exact, portable unsigned arithmetic. */
    (void)llg_rng_state_next(state);
    state->state += splitmix64(seed ^ UINT64_C(0x13198a2e03707344));
    (void)llg_rng_state_next(state);
}

void llg_rng_state_derive(llg_rng_state_t* child,
                          const llg_rng_state_t* parent,
                          uint64_t ordinal) {
    if (!child || !parent) rng_fail("deriving a stream from null state");
    /* Explicit ordinal-based stream splitting. Dynamic HDL children use
     * llg_rng_state_child instead, which consumes the parent's next draw. */
    uint64_t material = parent->increment ^
                        (ordinal * UINT64_C(0x9e3779b97f4a7c15));
    uint64_t seed = splitmix64(material ^ UINT64_C(0xa4093822299f31d0));
    llg_rng_state_seed(child, seed);
}

void llg_rng_state_child(llg_rng_state_t* parent, llg_rng_state_t* child) {
    if (!parent || !child || parent == child)
        rng_fail("creating a child requires two distinct streams");
    if (parent->child_count == UINT64_MAX) rng_fail("child counter overflow");
    /* IEEE 1800-2009 18.14: creation consumes exactly one parent draw.
     * Draws in an already-created child never advance the parent or siblings. */
    uint32_t seed = llg_rng_state_next(parent);
    parent->child_count++;
    llg_rng_state_seed(child, seed);
}

uint32_t llg_rng_state_next(llg_rng_state_t* state) {
    if (!state) rng_fail("drawing from a null stream");
    uint64_t old = state->state;
    state->state = old * UINT64_C(6364136223846793005) + state->increment;
    uint32_t xorshifted = (uint32_t)(((old >> 18) ^ old) >> 27);
    uint32_t rotation = (uint32_t)(old >> 59);
    return (xorshifted >> rotation) |
           (xorshifted << ((uint32_t)(-(int32_t)rotation) & 31u));
}

uint32_t llg_rng_state_uniform(llg_rng_state_t* state,
                               uint32_t first, uint32_t second) {
    uint32_t low = first < second ? first : second;
    uint32_t high = first < second ? second : first;
    uint64_t span = (uint64_t)high - (uint64_t)low + UINT64_C(1);
    /* Rejection over the complete 2^32-domain avoids modulo bias.  The
     * `span == 2^32` case is represented by 2^32 in uint64_t and naturally
     * accepts every draw. */
    uint64_t remainder = (UINT64_C(1) << 32) % span;
    uint32_t value;
    do {
        value = llg_rng_state_next(state);
    } while ((uint64_t)value < remainder);
    return low + (uint32_t)((uint64_t)value % span);
}

llg_string_t llg_rng_state_get(const llg_rng_state_t* state) {
    if (!state) rng_fail("serializing a null stream");
    char text[64];
    int length = snprintf(text, sizeof(text), "LLG_RNG_V1:%016" PRIx64
                          ":%016" PRIx64 ":%016" PRIx64,
                          state->state, state->increment, state->child_count);
    if (length < 0 || (size_t)length >= sizeof(text))
        rng_fail("random state serialization overflow");
    return llg_string_bytes(text, (size_t)length);
}

static int parse_hex(const char* text, size_t length, uint64_t* result) {
    uint64_t value = 0;
    for (size_t index = 0; index < length; index++) {
        unsigned digit;
        unsigned char c = (unsigned char)text[index];
        if (c >= '0' && c <= '9') digit = (unsigned)(c - '0');
        else if (c >= 'a' && c <= 'f') digit = (unsigned)(c - 'a' + 10);
        else if (c >= 'A' && c <= 'F') digit = (unsigned)(c - 'A' + 10);
        else return 0;
        value = (value << 4) | digit;
    }
    *result = value;
    return 1;
}

int llg_rng_state_set(llg_rng_state_t* state, const llg_string_t* encoded) {
    static const char prefix[] = "LLG_RNG_V1:";
    const size_t field = 16;
    const size_t prefix_length = sizeof(prefix) - 1;
    const size_t expected = prefix_length + field * 3 + 2;
    if (!state || !encoded || encoded->len != expected || !encoded->data ||
        memcmp(encoded->data, prefix, prefix_length) != 0 ||
        encoded->data[prefix_length + field] != ':' ||
        encoded->data[prefix_length + field * 2 + 1] != ':')
        return 0;
    uint64_t next_state;
    uint64_t increment;
    uint64_t child_count;
    if (!parse_hex(encoded->data + prefix_length, field, &next_state) ||
        !parse_hex(encoded->data + prefix_length + field + 1, field, &increment) ||
        !parse_hex(encoded->data + prefix_length + field * 2 + 2, field,
                   &child_count) || !(increment & UINT64_C(1)))
        return 0;
    state->state = next_state;
    state->increment = increment;
    state->child_count = child_count;
    return 1;
}
