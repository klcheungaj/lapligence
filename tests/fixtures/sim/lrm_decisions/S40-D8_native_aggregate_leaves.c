/* Companion for S40-D8_native_aggregate_leaves.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

typedef struct {
    double r;
    int i;
    const char *s;
    float f;
    char b;
} leaf_t;

static void show(const char *tag, const leaf_t *v)
{
    printf("%s: %g %d [%s] %g %d\n", tag, v->r, v->i, v->s ? v->s : "<null>", (double)v->f,
           (int)(signed char)v->b);
}

DPI_DLLESPEC void d8_leaf(const leaf_t *i, leaf_t *o, leaf_t *io)
{
    show("i", i);
    show("o", o);
    show("io", io);
    o->r = 6.5;
    o->i = 66;
    o->s = "out";
    o->f = 1.75f;
    o->b = (char)-5;
    io->r += 1.0;
    io->i += 1;
    io->f *= 2.0f;
    io->b = (char)7;
}

DPI_DLLESPEC void d8_strings(const char **a, const char **o, const char **io)
{
    printf("a: %s %s o: [%s] [%s] io: %s %s %s\n", a[0], a[1], o[0] ? o[0] : "<null>",
           o[1] ? o[1] : "<null>", io[0], io[1], io[2]);
    o[0] = "out0";
    o[1] = "out1";
    /* Rotate two leaves through each other's characters; the third keeps
     * its pointer. */
    {
        const char *first = io[0];
        io[0] = io[1];
        io[1] = first;
    }
}

DPI_DLLESPEC void d8_reals(const double *a, double *io)
{
    printf("a: %g %g %g io: %g %g\n", a[0], a[1], a[2], io[0], io[1]);
    io[0] += a[0];
    io[1] += a[2];
}
