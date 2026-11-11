/* SIM-040 companion for native_open_arrays.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

typedef struct {
    double r;
    const char *s;
    int i;
} rec_t;

static int ids[16];

DPI_DLLESPEC void *no_handle(int id)
{
    ids[id] = id;
    return &ids[id];
}

DPI_DLLESPEC int no_id(void *h)
{
    return h ? *(int *)h : -1;
}

static void shape(const char *tag, const svOpenArrayHandle h)
{
    int d;
    printf("c %s dims=%d", tag, svDimensions(h));
    for (d = 0; d <= svDimensions(h); d++) {
        printf(" d%d=[%d:%d],%d", d, svLeft(h, d), svRight(h, d), svSize(h, d));
    }
    printf("\n");
}

DPI_DLLESPEC double no_sum(const char *tag, const svOpenArrayHandle a)
{
    /* The whole array is a C array of double, lowest index first (H.7.3). */
    const double *raw = (const double *)svGetArrayPtr(a);
    double sum = 0.0;
    int i;
    shape(tag, a);
    printf("c %s bytes=%d per=%d raw0=%g low=%g high=%g out=%d\n", tag, svSizeOfArray(a),
           (int)(svSizeOfArray(a) / (int)sizeof(double)), raw[0],
           *(const double *)svGetArrElemPtr1(a, svLow(a, 1)),
           *(const double *)svGetArrElemPtr1(a, svHigh(a, 1)),
           svGetArrElemPtr1(a, svHigh(a, 1) + 1) != NULL);
    for (i = svLow(a, 1); i <= svHigh(a, 1); i++) {
        sum += *(const double *)svGetArrElemPtr1(a, i);
    }
    return sum;
}

DPI_DLLESPEC void no_scale(const svOpenArrayHandle a, const svOpenArrayHandle by)
{
    const float *factor = (const float *)svGetArrayPtr(by);
    int i;
    shape("by", by);
    printf("c by bytes=%d factors=%g %g\n", svSizeOfArray(by), (double)factor[0],
           (double)factor[1]);
    for (i = 0; i < svSize(a, 1); i++) {
        *(double *)svGetArrElemPtr1(a, svLow(a, 1) + i) *= (double)factor[i];
    }
}

DPI_DLLESPEC void no_grid(const char *tag, const svOpenArrayHandle g)
{
    int i;
    int j;
    shape(tag, g);
    for (i = svLow(g, 1); i <= svHigh(g, 1); i++) {
        for (j = svLow(g, 2); j <= svHigh(g, 2); j++) {
            double *cell = (double *)svGetArrElemPtr2(g, i, j);
            printf("c %s[%d][%d]=%g\n", tag, i, j, *cell);
            *cell += 0.5;
        }
    }
    printf("c %s raw last=%g\n", tag, ((const double *)svGetArrayPtr(g))[5]);
}

DPI_DLLESPEC void no_rows(const char *tag, const svOpenArrayHandle g)
{
    /* The second dimension is sized in the formal: C sees it normalized. */
    shape(tag, g);
    printf("c %s corner=%g %g\n", tag, *(const double *)svGetArrElemPtr2(g, 0, 0),
           *(const double *)svGetArrElemPtr2(g, 2, 1));
}

DPI_DLLESPEC void no_strings(const svOpenArrayHandle a, const svOpenArrayHandle o,
                             const svOpenArrayHandle io)
{
    const char **raw = (const char **)svGetArrayPtr(a);
    shape("strings a", a);
    shape("strings o", o);
    shape("strings io", io);
    printf("c strings a=%s %s %s bytes=%d\n", raw[0], raw[1], raw[2],
           (int)(svSizeOfArray(a) / (int)sizeof(const char *)));
    printf("c strings o=[%s] [%s]\n", *(const char **)svGetArrElemPtr1(o, 4),
           *(const char **)svGetArrElemPtr1(o, 5));
    printf("c strings io=%s %s %s %s\n", *(const char **)svGetArrElemPtr2(io, 0, 0),
           *(const char **)svGetArrElemPtr2(io, 0, 1), *(const char **)svGetArrElemPtr2(io, 1, 0),
           *(const char **)svGetArrElemPtr2(io, 1, 1));
    *(const char **)svGetArrElemPtr1(o, 5) = raw[2];
    *(const char **)svGetArrElemPtr1(o, 4) = "four";
    *(const char **)svGetArrElemPtr2(io, 1, 0) = *(const char **)svGetArrElemPtr2(io, 0, 1);
    *(const char **)svGetArrElemPtr2(io, 0, 0) = "zero";
}

DPI_DLLESPEC void no_handles(const svOpenArrayHandle a, const svOpenArrayHandle o)
{
    void **in = (void **)svGetArrayPtr(a);
    shape("handles o", o);
    printf("c handles a=%d %d o=%d %d\n", no_id(in[0]), no_id(in[1]),
           no_id(*(void **)svGetArrElemPtr1(o, 0)), no_id(*(void **)svGetArrElemPtr1(o, 1)));
    *(void **)svGetArrElemPtr1(o, 1) = in[0];
    *(void **)svGetArrElemPtr1(o, 0) = in[1];
}

DPI_DLLESPEC void no_records(const svOpenArrayHandle a, const svOpenArrayHandle io)
{
    const rec_t *in = (const rec_t *)svGetArrayPtr(a);
    int i;
    shape("records a", a);
    printf("c records per=%d a=%g %s %d / %g %s %d\n", (int)(svSizeOfArray(a) / (int)sizeof(rec_t)),
           in[0].r, in[0].s, in[0].i, in[1].r, in[1].s, in[1].i);
    for (i = 0; i <= 1; i++) {
        rec_t *slot = (rec_t *)svGetArrElemPtr1(io, i);
        const rec_t *from = (const rec_t *)svGetArrElemPtr1(a, 2 - i);
        printf("c records io[%d]=%g %s %d\n", i, slot->r, slot->s, slot->i);
        slot->r += from->r;
        slot->s = from->s;
        slot->i = -slot->i;
    }
}
