/* SIM-041 companion for chains.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>
#include <string.h>

int ch_step(int i, int *sq, int *total);
int ch_down(int n);
void ch_pair(int a, int *x, int *y);
const char *ch_text(const char *s, const char **o, const char **io);
void ch_wide(const svLogicVecVal *a, svLogicVecVal *b, svBitVecVal *c);
char ch_byte(char b, char *o);
int ch_tick(int k, int *o);

int ch_loop(int n)
{
    int total = 0;
    int returned = 0;
    int bad = 0;
    int i;
    char first[3][16];
    for (i = 0; i < n; i++) {
        int sq = -1;
        returned += ch_step(i, &sq, &total);
        if (sq != i * i) {
            bad++;
        }
    }
    printf("c steps total=%d returned=%d bad=%d\n", total, returned, bad);
    for (i = 0; i < 200; i++) {
        const char *o = NULL;
        const char *io = "seed";
        const char *r = ch_text("abc", &o, &io);
        if (i == 0) {
            /* Strings returned by an export stay valid until the next call
             * into SystemVerilog; copy them before that. */
            snprintf(first[0], sizeof(first[0]), "%s", o ? o : "(null)");
            snprintf(first[1], sizeof(first[1]), "%s", io ? io : "(null)");
            snprintf(first[2], sizeof(first[2]), "%s", r ? r : "(null)");
        }
    }
    printf("c text o=%s io=%s ret=%s\n", first[0], first[1], first[2]);
    {
        svLogicVecVal a[4];
        svLogicVecVal b[4];
        svBitVecVal c[3];
        a[0].aval = 0x0000000fu; a[0].bval = 0x000000f0u;
        a[1].aval = 0x12345678u; a[1].bval = 0u;
        a[2].aval = 0x9abcdef0u; a[2].bval = 0u;
        a[3].aval = 0x5u;        a[3].bval = 0x2u;
        c[0] = 0xffffffffu; c[1] = 0xffffffffu; c[2] = 0u;
        ch_wide(a, b, c);
        printf("c wide b=%x/%x %08x %08x %x/%x c=%x %08x %08x\n", (unsigned)(b[0].aval & 0xffu),
               (unsigned)(b[0].bval & 0xffu), (unsigned)b[1].aval, (unsigned)b[2].aval,
               (unsigned)(b[3].aval & 0xfu), (unsigned)(b[3].bval & 0xfu), (unsigned)(c[2] & 1u),
               (unsigned)c[1], (unsigned)c[0]);
    }
    {
        char o = 0;
        char r = ch_byte((char)-5, &o);
        printf("c byte r=%d o=%d\n", (int)(signed char)r, (int)(signed char)o);
    }
    return total;
}

int ch_rec(int n)
{
    return ch_down(n);
}

/* One address for both outputs of an export: copy-back writes them in
 * declaration order, so the last output wins. */
void ch_alias(void)
{
    int v = 9;
    ch_pair(3, &v, &v);
    printf("c alias v=%d\n", v);
}

int ch_task(int n, int *o)
{
    int i;
    int sum = 0;
    for (i = 1; i <= n; i++) {
        int t = 0;
        if (ch_tick(i, &t) != 0) {
            printf("c unexpected disable\n");
            return 1;
        }
        sum += t;
    }
    *o = sum;
    return 0;
}
