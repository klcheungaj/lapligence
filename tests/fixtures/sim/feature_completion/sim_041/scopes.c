/* SIM-041 companion for scopes.sv. Portable C11. */
#include "svdpi.h"
#include <stdio.h>

int cell_value(int k);
void cell_bump(int d);
int cell_nest(int k);
int plain_twice(int k);

static const char *const cell_names[4] = {"tb.u1", "tb.u2", "tb.g[0].c", "tb.g[1].c"};

static const char *scope_name(svScope scope)
{
    const char *name = svGetNameFromScope(scope);
    return name ? name : "(null)";
}

/* Scope lookup, naming and per-scope user data. */
int tb_scopes(void)
{
    static int key_a;
    static int key_b;
    static int data[4];
    svScope self = svGetScope();
    int found = 0;
    int i;
    printf("c tb_scopes in %s\n", scope_name(self));
    for (i = 0; i < 4; i++) {
        svScope scope = svGetScopeFromName(cell_names[i]);
        if (scope) {
            found++;
        }
        printf("c name %s -> %s\n", cell_names[i], scope_name(scope));
        data[i] = 10 * (i + 1);
        if (svPutUserData(scope, &key_a, &data[i]) != 0) {
            printf("c put failed\n");
        }
    }
    printf("c unknown -> %s\n", svGetScopeFromName("tb.nope") ? "found" : "NULL");
    printf("c tb -> %s\n", scope_name(svGetScopeFromName("tb")));
    for (i = 0; i < 4; i++) {
        svScope scope = svGetScopeFromName(cell_names[i]);
        int *a = (int *)svGetUserData(scope, &key_a);
        void *b = svGetUserData(scope, &key_b);
        printf("c user %s a=%d b=%s\n", cell_names[i], a ? *a : -1, b ? "set" : "NULL");
    }
    /* Replacing a key's data keeps one entry per key and scope. */
    svPutUserData(svGetScopeFromName("tb.u1"), &key_a, &data[3]);
    printf("c replaced a=%d\n", *(int *)svGetUserData(svGetScopeFromName("tb.u1"), &key_a));
    printf("c put errors %d %d %d\n", svPutUserData(NULL, &key_a, &data[0]),
           svPutUserData(self, &key_a, NULL), svPutUserData(self, NULL, &data[0]));
    return found;
}

/* One exported symbol, every instance selected with svSetScope. */
int tb_drive(void)
{
    svScope self = svGetScope();
    int total = 0;
    int i;
    for (i = 0; i < 4; i++) {
        svScope scope = svGetScopeFromName(cell_names[i]);
        svScope previous = svSetScope(scope);
        int v;
        if (previous != self) {
            printf("c previous scope is not the caller scope\n");
        }
        v = cell_value(i);
        cell_bump(10 * (i + 1));
        printf("c %s value(%d)=%d after bump %d\n", cell_names[i], i, v, cell_value(0));
        total += v;
        svSetScope(self);
    }
    /* Nested re-entry: the export calls a context import of its instance,
     * which switches to another instance before returning. */
    svSetScope(svGetScopeFromName("tb.u1"));
    total += cell_nest(3);
    printf("c after nest scope %s\n", scope_name(svGetScope()));
    svSetScope(self);
    printf("c restored scope %s\n", scope_name(svGetScope()));
    return total;
}

int cell_visit(int k)
{
    svScope self = svGetScope();
    int v;
    printf("c cell_visit(%d) in %s\n", k, scope_name(self));
    svSetScope(svGetScopeFromName("tb.u2"));
    v = cell_value(k);
    printf("c cell_visit read u2 %d, scope now %s\n", v, scope_name(svGetScope()));
    /* Noncontext and pure imports are ordinary C calls here, so calling
     * them directly needs no context. */
    return v + plain_twice(1);
}

/* A noncontext import has no scope even inside a context chain. */
int cell_peek(void)
{
    printf("c cell_peek scope %s\n", svGetScope() ? "set" : "NULL");
    return 1;
}

int plain_twice(int k)
{
    return 2 * k;
}

int pure_add(int a, int b)
{
    return a + b;
}
