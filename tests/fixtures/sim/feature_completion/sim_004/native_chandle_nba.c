/* SIM-004 companion for native_chandle_nba.sv: foreign objects that only this
 * library allocates and frees. A queued chandle is a borrowed pointer, so the
 * simulator must neither free nor copy the pointee. */
#include <stdint.h>
#include <stdlib.h>

typedef struct {
    uint32_t magic;
    int32_t value;
} foreign_t;

enum { FOREIGN_MAGIC = 0x51A4004u };

static int32_t live;
static int32_t bad;

void* foreign_make(int32_t value) {
    foreign_t* object = malloc(sizeof(*object));
    if (!object) abort();
    object->magic = FOREIGN_MAGIC;
    object->value = value;
    ++live;
    return object;
}

int32_t foreign_value(void* handle) {
    foreign_t* object = handle;
    if (!object || object->magic != FOREIGN_MAGIC) {
        ++bad;
        return -1;
    }
    return object->value;
}

void foreign_release(void* handle) {
    foreign_t* object = handle;
    if (!object || object->magic != FOREIGN_MAGIC) {
        ++bad;
        return;
    }
    object->magic = 0;
    free(object);
    --live;
}

int32_t foreign_live(void) { return live; }

int32_t foreign_bad(void) { return bad; }
