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
    printf("put null scope %d\n", svPutUserData(NULL, &key, &first));
    printf("put null data %d\n", svPutUserData(self, &key, NULL));
    printf("put null key %d\n", svPutUserData(self, NULL, &first));
    printf("put %d\n", svPutUserData(self, &key, &first));
    printf("put again %d\n", svPutUserData(self, &key, &second));
    data = (int *)svGetUserData(self, &key);
    printf("get %d\n", data ? *data : -1);
    printf("get null scope %s\n", svGetUserData(NULL, &key) ? "set" : "NULL");
    printf("get other key %s\n", svGetUserData(self, &other_key) ? "set" : "NULL");
    printf("name of null %s\n", svGetNameFromScope(NULL) ? "set" : "NULL");
    printf("scope of null %s\n", svGetScopeFromName(NULL) ? "set" : "NULL");
    printf("set null returned %s\n", svSetScope(NULL) == self ? "the current scope" : "other");
    printf("scope after %s\n", svGetNameFromScope(svGetScope()));
    return 4;
}
