/* SIM-040 companion for native_aggregates.sv. Portable C11. Each structure
 * below is the C type H.7.8 gives the SystemVerilog one: members in
 * declaration order with the types of Table H.1, a string as const char*
 * (H.8.10.1), a 4-bit logic member as one canonical chunk (H.7.7). */
#include "svdpi.h"
#include <stdio.h>

typedef struct {
    double r;
    int i;
    const char *s;
    void *h;
    float f;
    char b;
} leaf_t;

typedef struct {
    int a;
    short q[2];
} plain_t;

typedef struct {
    leaf_t one;
    double pair[2];
    const char *names[2];
    svLogicVecVal tag;
    plain_t plain;
} nest_t;

typedef struct {
    const char *s;
    int big[20];
} wide_t;

static int ids[100];
/* Storage the foreign side owns for strings it hands back (H.8.10). */
static const char *const fresh[4] = {"new0", "new1", "new2", "new3"};

DPI_DLLESPEC void *na_handle(int id)
{
    ids[id] = id;
    return &ids[id];
}

DPI_DLLESPEC int na_id(void *h)
{
    return h ? *(int *)h : -1;
}

static const char *text(const char *s)
{
    return s ? s : "<null>";
}

static void show(const char *tag, const leaf_t *v)
{
    printf("c %s r=%g i=%d s=%s h=%d f=%g b=%d\n", tag, v->r, v->i, text(v->s), na_id(v->h),
           (double)v->f, (int)(signed char)v->b);
}

DPI_DLLESPEC void na_leaf(const leaf_t *i, leaf_t *o, leaf_t *io)
{
    show("leaf i", i);
    show("leaf o", o);
    show("leaf io", io);
    o->r = 6.5;
    o->i = 66;
    o->s = fresh[0];
    o->h = na_handle(33);
    o->f = 1.75f;
    o->b = (char)-5;
    io->r += 1.0;
    io->i += 1;
    /* io->s keeps its pointer: the string is unchanged (H.8.10). */
    io->h = na_handle(44);
    io->f *= 2.0f;
    io->b = (char)7;
}

DPI_DLLESPEC void na_nest(const nest_t *i, nest_t *io)
{
    show("nest i.one", &i->one);
    printf("c nest i pair=%g %g names=%s %s tag=%x/%x plain=%d %d %d\n", i->pair[0], i->pair[1],
           text(i->names[0]), text(i->names[1]), (unsigned)(i->tag.aval & 0xfu),
           (unsigned)(i->tag.bval & 0xfu), i->plain.a, (int)i->plain.q[0], (int)i->plain.q[1]);
    show("nest io.one", &io->one);
    io->one.s = fresh[1];
    io->pair[0] += 0.25;
    io->pair[1] += 0.5;
    /* One member takes another's characters; both pointers stay valid until
     * the call returns. */
    io->names[0] = io->names[1];
    io->names[1] = fresh[2];
    io->tag.aval = 0x6u;
    io->tag.bval = 0x3u;
    io->plain.a = -io->plain.a;
    io->plain.q[0] = (short)(io->plain.q[0] - 1);
    io->plain.q[1] = (short)(io->plain.q[1] + 1);
}

DPI_DLLESPEC void na_strings(const char **a, const char **o, const char **io, const char **rows)
{
    int k;
    printf("c strings a=%s %s o=[%s] [%s]\n", text(a[0]), text(a[1]), text(o[0]), text(o[1]));
    printf("c strings io=%s %s %s %s\n", text(io[0]), text(io[1]), text(io[2]), text(io[3]));
    printf("c strings rows=");
    for (k = 0; k < 6; k++) {
        printf("%s%s", text(rows[k]), k == 5 ? "\n" : " ");
    }
    /* o is [3:2]: C index 0 is element 2. */
    o[0] = fresh[2];
    o[1] = fresh[3];
    io[1] = a[0];
    io[2] = io[3];
}

DPI_DLLESPEC void na_reals(const double *a, double *o, float *io)
{
    /* a is [3:1]: C index 0 is element 1. */
    printf("c reals a=%g %g %g o=%g %g %g %g io=%g %g\n", a[0], a[1], a[2], o[0], o[1], o[2],
           o[3], (double)io[0], (double)io[1]);
    o[0] = a[0] + 10.0;
    o[1] = a[1] + 10.0;
    o[2] = a[2] + 10.0;
    o[3] = -1.0;
    io[0] += 0.25f;
    io[1] += 0.25f;
}

DPI_DLLESPEC void na_handles(void **a, void **o, void **io)
{
    printf("c handles a=%d %d o=%d %d io=%d %d\n", na_id(a[0]), na_id(a[1]), na_id(o[0]),
           na_id(o[1]), na_id(io[0]), na_id(io[1]));
    /* o is [1:0]: C index 0 is element 0. */
    o[0] = a[1];
    o[1] = a[0];
    io[1] = io[0];
    io[0] = NULL;
}

DPI_DLLESPEC void na_records(const leaf_t *a, leaf_t *o)
{
    /* a is [2:1]: C index 0 is element 1. */
    show("records a[1]", &a[0]);
    show("records a[2]", &a[1]);
    show("records o[0]", &o[0]);
    o[0] = a[1];
    o[1] = a[0];
    o[1].s = fresh[3];
}

DPI_DLLESPEC void na_order(int k, leaf_t *first, int *n, leaf_t *second)
{
    first->i = k;
    first->s = fresh[0];
    first->r = 1.0;
    *n = k + 1;
    second->i = k + 2;
    second->s = fresh[1];
    second->r = 2.0;
}

DPI_DLLESPEC int na_task(const char **a, double *o, leaf_t *io)
{
    printf("c task a=%s %s io.s=%s\n", text(a[0]), text(a[1]), text(io->s));
    o[0] = 100.0;
    o[1] = 200.0;
    io->i = 1000;
    io->s = a[1];
    return 0;
}

DPI_DLLESPEC void na_wide(wide_t *a, const svOpenArrayHandle copy)
{
    const wide_t *raw = (const wide_t *)svGetArrayPtr(copy);
    printf("c wide a=%s %d %d %s %d copy=%s %d size=%d\n", text(a[0].s), a[0].big[1], a[0].big[19],
           text(a[1].s), a[1].big[19], text(raw[1].s), raw[1].big[0], svSize(copy, 1));
    a[0].big[19] = 500;
    a[1].s = a[0].s;
}
