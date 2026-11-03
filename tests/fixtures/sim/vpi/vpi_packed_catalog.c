/* Packed catalog rows (llg_vpi_packed_t): a hand-encoded blob decodes into
 * the object array on the first VPI use, and every corruption the decoder
 * bounds-checks fails that use with an error instead of reading past the
 * blob. The stream layout is the one src/sim/rt/io.md documents. */
#include "llg_vpi.h"
#include <stdio.h>
#include <string.h>

#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "VPI packed catalog check failed at line %d\n", __LINE__); \
        return 1; \
    } \
} while (0)

#define ROWS 2u
static char blob[ROWS][LLG_VPI_PACK_ROW];
static size_t length;

static void put(char byte) {
    blob[length / (LLG_VPI_PACK_ROW - 1u)][length % (LLG_VPI_PACK_ROW - 1u)] = byte;
    ++length;
}

static void u(uint64_t value) {
    static const char digits[] =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (;;) {
        unsigned group = (unsigned)(value & 31u);
        value >>= 5;
        if (!value) {
            put(digits[group]);
            return;
        }
        put(digits[group | 32u]);
    }
}

static void z(int64_t value) { u(((uint64_t)value << 1) ^ (uint64_t)(value >> 63)); }

/* One object row: `parent` and the reference mask vary, the rest is plain. */
static void row(uint64_t type, uint64_t name, uint64_t full_name, uint64_t line,
                uint64_t width, uint64_t parent, uint64_t flags, uint64_t mask) {
    u(type);
    u(name);
    u(full_name);
    u(0); /* definition name */
    u(0); /* file */
    u(line);
    u(width);
    u(0); /* is_signed | is_real << 1 | is_net << 2 */
    u(parent);
    u(0); /* time unit */
    u(2); /* subtype: symbol 0 */
    u(flags);
    z(0); /* left */
    z(0); /* right */
    u(0); /* dimensions */
    u(0); /* port index */
    u(0); /* target */
    u(0); /* constant */
    u(0); /* real constant bits */
    u(0); /* typespec */
    u(2); /* detail: symbol 0 */
    u(0); /* connections */
    u(0); /* activation declaration */
    u(0); /* activation instance */
    u(0); /* timing */
    u(0); /* shape */
    u(0); /* code */
    u(0); /* instance */
    u(0); /* strength */
    u(mask);
}

static llg_vpi_model_object_t objects[2];
static sv4_t cell = SV4_EMPTY;

/* Initialization only records the blob; the first use decodes it and
 * reports a corrupt one through vpi_chk_error(). */
static int rejected(const llg_vpi_packed_t* packed) {
    if (!llg_vpi_model_init("tb", objects, 2) || !llg_vpi_model_packed(packed)) return 0;
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL);
    int ok = handle == NULL && vpi_chk_error(NULL) == vpiError;
    llg_vpi_shutdown();
    return ok;
}

int main(void) {
    llg_rt_init();
    static const char pool[] = "tb\0r\0tb.r";
    for (size_t i = 0; i < sizeof(pool); ++i) put(pool[i]);
    uint32_t pool_bytes = (uint32_t)sizeof(pool);
    /* Totals of connections, dimension pairs, instances, timings and
     * declared delays. */
    for (int i = 0; i < 5; ++i) u(0);
    row(0, 1, 1, 0, 0, 0, 3, 0);
    row(1, 4, 6, 7, 8, 1, 2, 1);
    uint32_t object_bytes = (uint32_t)(length - pool_bytes);
    static const int64_t symbols[] = {vpiModule, vpiReg, 0, LLG_VPI_OBJ_TOP};
    const void* const refs[] = {&cell};
    const llg_vpi_packed_t valid = {&blob[0][0], 1u, pool_bytes, object_bytes, 0u,
                                    symbols, 4u, refs, 1u};

    CHECK(llg_vpi_model_init("tb", objects, 2));
    CHECK(llg_vpi_model_packed(&valid));
    /* Registration alone leaves the rows zero. */
    CHECK(objects[1].name == NULL);
    vpiHandle reg = vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL);
    CHECK(reg != NULL);
    CHECK(vpi_get(vpiType, reg) == vpiReg);
    CHECK(vpi_get(vpiSize, reg) == 8);
    CHECK(vpi_get(vpiLineNo, reg) == 7);
    CHECK(strcmp(vpi_get_str(vpiName, reg), "r") == 0);
    CHECK(vpi_get(vpiTopModule, vpi_handle(vpiModule, reg)) == 1);
    CHECK(objects[1].packed == &cell && objects[1].parent == &objects[0]);
    CHECK(!llg_vpi_failed());
    llg_vpi_shutdown();

    llg_vpi_packed_t bad = valid;
    bad.object_bytes = object_bytes - 1u; /* the last varint is cut off */
    CHECK(rejected(&bad));
    bad = valid;
    bad.symbol_count = 3u; /* LLG_VPI_OBJ_TOP out of range */
    CHECK(rejected(&bad));
    bad = valid;
    bad.ref_count = 0u; /* the reg's storage reference is missing */
    CHECK(rejected(&bad));
    bad = valid;
    bad.pool_bytes = pool_bytes - 1u; /* the pool does not end in a NUL */
    CHECK(rejected(&bad));
    bad = valid;
    bad.code_bytes = 1u; /* code rows without code tables */
    CHECK(rejected(&bad));
    bad = valid;
    bad.chunk_count = 0u; /* the payload does not fit the blob */
    CHECK(rejected(&bad));
    put('A'); /* a stray value after the last row */
    bad = valid;
    bad.object_bytes = object_bytes + 1u;
    CHECK(rejected(&bad));

    /* A later model decodes again into the same rows. */
    CHECK(llg_vpi_model_init("tb", objects, 2));
    CHECK(llg_vpi_model_packed(&valid));
    CHECK(vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL) != NULL);
    CHECK(!llg_vpi_failed());
    llg_vpi_shutdown();
    llg_rt_cleanup();
    puts("vpi packed catalog ok");
    return 0;
}
