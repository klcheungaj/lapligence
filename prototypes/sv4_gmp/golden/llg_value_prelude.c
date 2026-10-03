// llg_value.c — four-state values and numeric conversions for generated C11 models.

#include "llg_value.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

_Static_assert(sizeof(double) == sizeof(uint64_t),
               "$realtobits requires a 64-bit C double");
_Static_assert(sizeof(float) == sizeof(uint32_t),
               "$shortrealtobits requires a 32-bit C float");
