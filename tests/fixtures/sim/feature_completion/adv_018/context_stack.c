/* ADV-018 companion for context_stack.sv. Portable C11. */
#include "svdpi.h"
#include <stddef.h>

int cs_wait(int delay);

#define CS_PAGE 4096u

/* Write one byte per page from the top of the frame down, so a frame larger
 * than the stack faults at its guard page instead of jumping past it, and
 * count the pages. */
static int cs_fill(volatile unsigned char* frame, size_t size)
{
    int pages = 0;
    for (size_t at = size; at >= CS_PAGE; at -= CS_PAGE) {
        frame[at - 1] = 1;
        pages += frame[at - 1];
    }
    return pages;
}

static int cs_frame_6m(void)
{
    volatile unsigned char frame[6u << 20];
    return cs_fill(frame, sizeof(frame));
}

static int cs_frame_12m(void)
{
    volatile unsigned char frame[12u << 20];
    return cs_fill(frame, sizeof(frame));
}

/* Called through volatile pointers so neither frame is inlined into the
 * other's caller. */
static int (*volatile cs_small)(void) = cs_frame_6m;
static int (*volatile cs_big)(void) = cs_frame_12m;

DPI_DLLESPEC int cs_deep(int big, int* pages)
{
    *pages = big ? cs_big() : cs_small();
    return cs_wait(5);
}
