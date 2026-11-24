/* FIN-002 DPI-C companion of tb_fifo_env.sv: a bounded FIFO model behind a
 * chandle. A refused request is reported through the exported sv_note. */
#include "svdpi.h"
#include <stdlib.h>

extern void sv_note(const char* what);

typedef struct {
    int depth;
    int count;
    int latched;
    int head;
    int values[16];
} model_t;

void* c_model_new(int depth)
{
    model_t* model = calloc(1, sizeof(model_t));
    if (model) model->depth = depth > 16 ? 16 : depth;
    return model;
}

int c_model_push(void* handle, int value)
{
    model_t* model = handle;
    if (model->latched == model->depth) {
        sv_note("refused push");
        return 0;
    }
    model->values[(model->head + model->latched) % model->depth] = value;
    model->count++;
    return 1;
}

int c_model_pop(void* handle, int* value)
{
    model_t* model = handle;
    if (model->latched == 0) {
        sv_note("refused pop");
        *value = 0;
        return 0;
    }
    *value = model->values[model->head];
    model->head = (model->head + 1) % model->depth;
    model->count--;
    return 1;
}

/* The fill level both requests of one clock edge are decided from. */
int c_model_latch(void* handle)
{
    model_t* model = handle;
    model->latched = model->count;
    return model->count;
}

void c_model_free(void* handle)
{
    free(handle);
}
