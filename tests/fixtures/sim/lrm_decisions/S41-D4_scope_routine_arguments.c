/* Companion of S41-D4_scope_routine_arguments.sv. */
#include "svdpi.h"
#include <stdio.h>

static int key;
static int other_key;
static int first = 1;
static int second = 2;

DPI_DLLESPEC int d4_run(void)
{
    svScope self = svGetScope();
    int *data;
    printf("put %d\n", svPutUserData(self, &key, &first));
    printf("put again %d\n", svPutUserData(self, &key, &second));
    data = (int *)svGetUserData(self, &key);
    printf("get %d\n", data ? *data : -1);
    printf("get other key %s\n", svGetUserData(self, &other_key) ? "set" : "NULL");
    printf("scope after %s\n", svGetNameFromScope(svGetScope()));
    return 4;
}
