/* SIM-040 companion for dynamic_actuals.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

extern void da_grow(int size);

typedef struct {
    int a;
    svLogicVecVal t;
} packed_rec_t;

typedef struct {
    double r;
    const char *s;
} native_rec_t;

static void shape(const char *tag, const svOpenArrayHandle h)
{
    int d;
    printf("c %s dims=%d", tag, svDimensions(h));
    for (d = 1; d <= svDimensions(h) - (svSize(h, 0) ? 1 : 0); d++) {
        printf(" [%d:%d] low=%d high=%d inc=%d size=%d", svLeft(h, d), svRight(h, d), svLow(h, d),
               svHigh(h, d), svIncrement(h, d), svSize(h, d));
    }
    printf(" bytes=%d\n", svSizeOfArray(h));
}

DPI_DLLESPEC int da_sum(const char *tag, const svOpenArrayHandle a)
{
    const int *raw = (const int *)svGetArrayPtr(a);
    int sum = 0;
    int i;
    shape(tag, a);
    for (i = svLow(a, 1); i <= svHigh(a, 1); i++) {
        sum += *(const int *)svGetArrElemPtr1(a, i);
    }
    /* No element lies outside [low:high]; an empty array has none at all. */
    printf("c %s below=%d above=%d first=%d\n", tag, svGetArrElemPtr1(a, svLow(a, 1) - 1) != NULL,
           svGetArrElemPtr1(a, svHigh(a, 1) + 1) != NULL, svSize(a, 1) ? raw[0] : -1);
    return sum;
}

DPI_DLLESPEC int da_pure(const svOpenArrayHandle a)
{
    int sum = 0;
    int i;
    for (i = 0; i < svSize(a, 1); i++) {
        sum += *(const int *)svGetArrElemPtr1(a, i);
    }
    return sum;
}

DPI_DLLESPEC void da_add(const svOpenArrayHandle a, int by)
{
    int i;
    for (i = svLow(a, 1); i <= svHigh(a, 1); i++) {
        *(int *)svGetArrElemPtr1(a, i) += by;
    }
}

DPI_DLLESPEC void da_resize(const svOpenArrayHandle a)
{
    shape("resize before", a);
    *(int *)svGetArrElemPtr1(a, 0) = 555;
    /* The exported function grows the actual to five elements. The formal
     * is a copy made at the call: it keeps its three. */
    da_grow(5);
    shape("resize after", a);
    printf("c resize first=%d fifth=%d\n", *(const int *)svGetArrElemPtr1(a, 0),
           svGetArrElemPtr1(a, 4) != NULL);
    *(int *)svGetArrElemPtr1(a, 1) = 666;
}

DPI_DLLESPEC void da_logic(const svOpenArrayHandle a)
{
    svLogicVecVal v;
    int i;
    shape("logic", a);
    printf("c logic packed=[%d:%d]\n", svLeft(a, 0), svRight(a, 0));
    for (i = svLow(a, 1); i <= svHigh(a, 1); i++) {
        svGetLogicArrElem1VecVal(&v, a, i);
        printf("c logic[%d]=%x/%x\n", i, (unsigned)(v.aval & 0xffu), (unsigned)(v.bval & 0xffu));
        v.aval = 0x5au + (unsigned)i;
        v.bval = 0x0fu;
        svPutLogicArrElem1VecVal(a, &v, i);
    }
}

DPI_DLLESPEC void da_packed(const char *tag, const svOpenArrayHandle a)
{
    svBitVecVal v;
    int i;
    shape(tag, a);
    printf("c %s packed=[%d:%d]", tag, svLeft(a, 0), svRight(a, 0));
    for (i = svLow(a, 1); i <= svHigh(a, 1); i++) {
        svGetBitArrElem1VecVal(&v, a, i);
        printf(" %x", (unsigned)v);
    }
    printf("\n");
}

DPI_DLLESPEC void da_reals(const svOpenArrayHandle a, const svOpenArrayHandle by)
{
    double *values = (double *)svGetArrayPtr(a);
    const float *factors = (const float *)svGetArrayPtr(by);
    int i;
    shape("reals", a);
    shape("factors", by);
    for (i = 0; i < svSize(a, 1); i++) {
        values[i] *= (double)factors[i];
    }
}

DPI_DLLESPEC void da_strings(const svOpenArrayHandle a)
{
    const char **text = (const char **)svGetArrayPtr(a);
    shape("strings", a);
    printf("c strings=%s %s %s\n", text[0], text[1], text[2]);
    /* Rotate: every pointer the foreign code stores stays valid until the
     * call returns. */
    {
        const char *first = text[0];
        text[0] = text[1];
        text[1] = text[2];
        text[2] = first;
    }
}

DPI_DLLESPEC void da_packed_recs(const svOpenArrayHandle a)
{
    int i;
    shape("packed recs", a);
    for (i = svLow(a, 1); i <= svHigh(a, 1); i++) {
        packed_rec_t *r = (packed_rec_t *)svGetArrElemPtr1(a, i);
        printf("c packed rec[%d]=%d %x/%x\n", i, r->a, (unsigned)(r->t.aval & 0xfu),
               (unsigned)(r->t.bval & 0xfu));
        r->a += 10;
        r->t.aval = 0xcu;
        r->t.bval = 0x1u;
    }
}

DPI_DLLESPEC void da_native_recs(const svOpenArrayHandle a)
{
    native_rec_t *r = (native_rec_t *)svGetArrayPtr(a);
    shape("native recs", a);
    printf("c native recs=%g %s %g %s\n", r[0].r, r[0].s, r[1].r, r[1].s);
    r[0].r += 10.0;
    r[1].r += 10.0;
    r[1].s = "N1";
}
