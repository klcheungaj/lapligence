/* SIM-040 A03 companion: calls each svdpi.h routine with valid, boundary
 * and invalid arguments and prints what it observes. Portable C11. */
#include "svdpi.h"
#include <stdio.h>
#include <string.h>

static void show_logic(const char *name, const svLogicVecVal *v, svBitVecVal mask)
{
    printf("%s=%x/%x\n", name, (unsigned)(v->aval & mask), (unsigned)(v->bval & mask));
}

void sa_version(void)
{
    const char *version = svDpiVersion();
    printf("version=%s\n", version ? version : "(null)");
}

void sa_select(svLogicVecVal *l, svBitVecVal *b)
{
    svLogicVecVal part;
    svBitVecVal bits;
    printf("bitsel l0=%d l3=%d l32=%d l39=%d b0=%d b39=%d b1=%d\n", (int)svGetBitselLogic(l, 0),
           (int)svGetBitselLogic(l, 3), (int)svGetBitselLogic(l, 32),
           (int)svGetBitselLogic(l, 39), (int)svGetBitselBit(b, 0), (int)svGetBitselBit(b, 39),
           (int)svGetBitselBit(b, 1));
    part.aval = 0u;
    part.bval = 0u;
    svGetPartselLogic(&part, l, 28, 8);
    show_logic("partsel l[35:28]", &part, 0xffu);
    part.aval = 0xffffffffu;
    part.bval = 0u;
    svGetPartselLogic(&part, l, 0, 32);
    show_logic("partsel l[31:0]", &part, 0xffffffffu);
    bits = 0xffffff00u;
    svGetPartselBit(&bits, b, 36, 4);
    printf("partsel b[39:36]=%x\n", (unsigned)bits);
    bits = 0u;
    svGetPartselBit(&bits, b, 0, 33);
    printf("partsel width33=%x\n", (unsigned)bits);

    svPutBitselLogic(l, 0, sv_z);
    svPutBitselLogic(l, 33, sv_1);
    svPutBitselLogic(l, 34, sv_0);
    part.aval = 0x5u;
    part.bval = 0x3u;
    svPutPartselLogic(l, part, 30, 4);
    svPutBitselBit(b, 39, sv_0);
    svPutBitselBit(b, 4, sv_1);
    svPutPartselBit(b, 0xabu, 28, 8);
    svPutPartselBit(b, 0xffu, -1, 8);
}

static void show_range(const svOpenArrayHandle h, int d)
{
    printf(" d%d=[%d:%d] lo=%d hi=%d inc=%d size=%d", d, svLeft(h, d), svRight(h, d), svLow(h, d),
           svHigh(h, d), svIncrement(h, d), svSize(h, d));
}

void sa_query(const svOpenArrayHandle c, const svOpenArrayHandle p, const svOpenArrayHandle s)
{
    int d;
    const svBitVecVal *packed;
    printf("c dims=%d bytes=%d ptr=%d\n", svDimensions(c), svSizeOfArray(c),
           svGetArrayPtr(c) != NULL);
    for (d = 0; d <= 4; d++) {
        printf("c");
        show_range(c, d);
        printf("\n");
    }
    printf("c invalid=%d,%d\n", svSize(c, -1), svLeft(c, 9));
    packed = (const svBitVecVal *)svGetArrayPtr(p);
    printf("p dims=%d bytes=%d value=%x elem=%d\n", svDimensions(p), svSizeOfArray(p),
           packed ? (unsigned)(packed[0] & 0xffu) : 0u, svGetArrElemPtr1(p, 0) != NULL);
    printf("p");
    show_range(p, 0);
    show_range(p, 1);
    printf("\n");
    printf("s dims=%d", svDimensions(s));
    show_range(s, 0);
    show_range(s, 1);
    printf(" v=%d%d%d\n", (int)svGetLogicArrElem1(s, 0), (int)svGetLogicArrElem1(s, 1),
           (int)svGetLogicArrElem1(s, 2));
}

void sa_pointers(const svOpenArrayHandle c)
{
    int *e;
    e = (int *)svGetArrElemPtr3(c, 1, 2, -1);
    printf("ptr3 c[1][2][-1]=%d\n", e ? *e : -999);
    e = (int *)svGetArrElemPtr(c, 0, 4, -2);
    printf("ptr c[0][4][-2]=%d\n", e ? *e : -999);
    /* Natural order (H.7.6 c): the lowest index of each dimension first. */
    printf("ptr first=%d last=%d\n",
           (int *)svGetArrElemPtr3(c, 0, 2, -2) == (int *)svGetArrayPtr(c),
           (int *)svGetArrElemPtr3(c, 1, 4, -1) == (int *)svGetArrayPtr(c) + 11);
    printf("ptr invalid=%d%d%d%d%d\n", svGetArrElemPtr3(c, 2, 2, -1) == NULL,
           svGetArrElemPtr3(c, 1, 5, -1) == NULL, svGetArrElemPtr3(c, 1, 2, 0) == NULL,
           svGetArrElemPtr2(c, 1, 2) == NULL, svGetArrElemPtr1(c, 1) == NULL);
    *(int *)svGetArrElemPtr3(c, 1, 3, -2) = -1;
    *(int *)svGetArrElemPtr(c, 0, 2, -1) = -2;
}

void sa_scalars(const svOpenArrayHandle l1, const svOpenArrayHandle l2,
                const svOpenArrayHandle l3, const svOpenArrayHandle b1,
                const svOpenArrayHandle b2, const svOpenArrayHandle b3)
{
    printf("l1 get=%d%d%d%d out=%d\n", (int)svGetLogicArrElem1(l1, 2), (int)svGetLogicArrElem1(l1, 1),
           (int)svGetLogicArrElem1(l1, 0), (int)svGetLogicArrElem(l1, 0),
           (int)svGetLogicArrElem1(l1, 3));
    printf("l2 get=%d%d%d%d out=%d\n", (int)svGetLogicArrElem2(l2, 0, 1),
           (int)svGetLogicArrElem2(l2, 0, 0), (int)svGetLogicArrElem2(l2, 1, 1),
           (int)svGetLogicArrElem(l2, 1, 0), (int)svGetLogicArrElem2(l2, 2, 0));
    printf("l3 get=%d%d%d%d out=%d\n", (int)svGetLogicArrElem3(l3, 1, 0, 2),
           (int)svGetLogicArrElem3(l3, 1, 0, 3), (int)svGetLogicArrElem3(l3, 0, 0, 2),
           (int)svGetLogicArrElem(l3, 0, 0, 3), (int)svGetLogicArrElem3(l3, 0, 1, 2));
    printf("b1 get=%d%d%d out=%d\n", (int)svGetBitArrElem1(b1, 0), (int)svGetBitArrElem1(b1, 1),
           (int)svGetBitArrElem(b1, 2), (int)svGetBitArrElem1(b1, -1));
    printf("b2 get=%d%d%d%d\n", (int)svGetBitArrElem2(b2, 1, 0), (int)svGetBitArrElem2(b2, 1, 1),
           (int)svGetBitArrElem2(b2, 0, 0), (int)svGetBitArrElem(b2, 0, 1));
    printf("b3 get=%d%d%d%d\n", (int)svGetBitArrElem3(b3, 0, 0, 3), (int)svGetBitArrElem3(b3, 0, 0, 2),
           (int)svGetBitArrElem3(b3, 1, 0, 3), (int)svGetBitArrElem(b3, 1, 0, 2));
    printf("bit view of logic=%d%d\n", (int)svGetBitArrElem1(l1, 0), (int)svGetBitArrElem2(l2, 0, 1));

    svPutLogicArrElem1(l1, sv_z, 2);
    svPutLogicArrElem(l1, sv_1, 0);
    svPutLogicArrElem1(l1, sv_1, 7);
    svPutLogicArrElem2(l2, sv_x, 0, 0);
    svPutLogicArrElem(l2, sv_0, 1, 0);
    svPutLogicArrElem3(l3, sv_1, 1, 0, 2);
    svPutLogicArrElem(l3, sv_z, 0, 0, 3);
    svPutLogicArrElem3(l3, sv_1, 0, 0, 4);
    svPutBitArrElem1(b1, sv_0, 0);
    svPutBitArrElem(b1, sv_1, 1);
    svPutBitArrElem2(b2, sv_0, 0, 1);
    svPutBitArrElem(b2, sv_1, 1, 0);
    svPutBitArrElem3(b3, sv_0, 0, 0, 2);
    svPutBitArrElem(b3, sv_1, 1, 0, 3);
    svPutBitArrElem3(b3, sv_1, 1, 1, 3);
}

void sa_vectors(const svOpenArrayHandle l1, const svOpenArrayHandle l2,
                const svOpenArrayHandle l3, const svOpenArrayHandle b1,
                const svOpenArrayHandle b2, const svOpenArrayHandle b3)
{
    svLogicVecVal lv;
    svBitVecVal bv[2];
    svGetLogicArrElem1VecVal(&lv, l1, 2);
    show_logic("l1[2]", &lv, 0x3ffu);
    svGetLogicArrElemVecVal(&lv, l1, 1);
    show_logic("l1[1]", &lv, 0x3ffu);
    svGetLogicArrElem1VecVal(&lv, l1, 0);
    show_logic("l1[0] out of range", &lv, 0x3ffu);
    svGetLogicArrElem2VecVal(&lv, l2, 1, 1);
    show_logic("l2[1][1]", &lv, 0x3ffu);
    svGetLogicArrElem3VecVal(&lv, l3, 0, 1, 1);
    show_logic("l3[0][1][1]", &lv, 0x3ffu);
    svGetLogicArrElemVecVal(&lv, l3, 0, 0, 1);
    show_logic("l3[0][0][1]", &lv, 0x3ffu);
    svGetBitArrElem1VecVal(bv, l1, 1);
    printf("bit view l1[1]=%x\n", (unsigned)(bv[0] & 0x3ffu));
    svGetBitArrElem1VecVal(bv, b1, 2);
    printf("b1[2]=%x%08x\n", (unsigned)(bv[1] & 0xfu), (unsigned)bv[0]);
    svGetBitArrElemVecVal(bv, b1, 1);
    printf("b1[1]=%x%08x\n", (unsigned)(bv[1] & 0xfu), (unsigned)bv[0]);
    svGetBitArrElem2VecVal(bv, b2, 1, 1);
    printf("b2[1][1]=%x%08x\n", (unsigned)(bv[1] & 0xfu), (unsigned)bv[0]);
    svGetBitArrElem3VecVal(bv, b3, 1, 1, 0);
    printf("b3[1][1][0]=%x%08x\n", (unsigned)(bv[1] & 0xfu), (unsigned)bv[0]);
    bv[0] = 7u;
    bv[1] = 7u;
    svGetBitArrElem3VecVal(bv, b3, 1, 2, 0);
    printf("b3 out of range=%x%08x\n", (unsigned)(bv[1] & 0xfu), (unsigned)bv[0]);

    lv.aval = 0x2a5u;
    lv.bval = 0x0f0u;
    svPutLogicArrElem1VecVal(l1, &lv, 1);
    lv.aval = 0x001u;
    lv.bval = 0x000u;
    svPutLogicArrElemVecVal(l1, &lv, 2);
    svPutLogicArrElem1VecVal(l1, &lv, 3);
    lv.aval = 0x155u;
    svPutLogicArrElem2VecVal(l2, &lv, 0, 1);
    lv.aval = 0x0aau;
    svPutLogicArrElemVecVal(l2, &lv, 1, 0);
    lv.aval = 0x3ffu;
    lv.bval = 0x3ffu;
    svPutLogicArrElem3VecVal(l3, &lv, 0, 0, 0);
    lv.bval = 0u;
    svPutLogicArrElemVecVal(l3, &lv, 0, 1, 1);
    bv[0] = 0xcafef00du;
    bv[1] = 0xfu;
    svPutBitArrElem1VecVal(b1, bv, 2);
    bv[1] = 0x3u;
    svPutBitArrElemVecVal(b1, bv, 1);
    bv[0] = 1u;
    bv[1] = 0u;
    svPutBitArrElem2VecVal(b2, bv, 0, 1);
    svPutBitArrElemVecVal(b2, bv, 1, 1);
    bv[0] = 0u;
    bv[1] = 0x8u;
    svPutBitArrElem3VecVal(b3, bv, 1, 0, 1);
    svPutBitArrElemVecVal(b3, bv, 1, 1, 0);
    svPutBitArrElem3VecVal(b3, bv, 0, 0, 0);
}
