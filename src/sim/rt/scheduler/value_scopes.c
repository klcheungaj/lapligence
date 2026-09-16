/* Scope references protect descriptor addresses, not just their limb payloads.
 * A queued NBA may outlive both lexical scope exit and its issuing process. */
static void value_scope_release(llg_value_scope_t* scope) {
    if (!scope) return;
    if (scope->references == 0) {
        fputs("llg runtime fatal: value scope reference underflow\n", stderr);
        abort();
    }
    if (--scope->references != 0) return;
    if (scope->all_prev) scope->all_prev->all_next = scope->all_next;
    else all_value_scopes = scope->all_next;
    if (scope->all_next) scope->all_next->all_prev = scope->all_prev;
    sv4_destroy_array(scope->values, scope->count);
    free(scope->values);
    free(scope);
}

static llg_value_scope_t* value_scope_retain_target(sv4_t* target) {
    if (!target) return NULL;
    for (llg_value_scope_t* scope = all_value_scopes; scope; scope = scope->all_next) {
        /* Equality is defined even for pointers into different C objects. */
        for (size_t i = 0; i < scope->count; ++i) {
            if (&scope->values[i] != target) continue;
            if (scope->references == SIZE_MAX) {
                fputs("llg runtime fatal: value scope reference overflow\n", stderr);
                abort();
            }
            ++scope->references;
            return scope;
        }
    }
    return NULL; /* Model/global storage is owned by generated model teardown. */
}

llg_value_scope_t* llg_value_scope_begin(size_t count) {
    llg_value_scope_t* scope = (llg_value_scope_t*)llg_checked_calloc(
        1, sizeof(*scope), "value owner scope");
    scope->values = count ? (sv4_t*)llg_checked_calloc(
        count, sizeof(sv4_t), "scoped values") : NULL;
    scope->count = count;
    scope->references = 1;
    scope->active = 1;
    scope->owner = llg_current();
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    scope->next = *head;
    *head = scope;
    scope->all_next = all_value_scopes;
    if (all_value_scopes) all_value_scopes->all_prev = scope;
    all_value_scopes = scope;
    return scope;
}

sv4_t* llg_value_scope_values(llg_value_scope_t* scope) {
    return scope ? scope->values : NULL;
}

void llg_value_scope_end(llg_value_scope_t* scope) {
    if (!scope) return;
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    while (*head && *head != scope) head = &(*head)->next;
    if (!scope->active || !*head) {
        fputs("llg runtime fatal: value scope is not registered\n", stderr);
        abort();
    }
    *head = scope->next;
    scope->next = NULL;
    scope->owner = NULL;
    scope->active = 0;
    value_scope_release(scope);
}

llg_value_scope_t* llg_value_scope_mark(void) {
    llg_proc_t* owner = llg_current();
    return owner ? owner->value_scopes : root_value_scopes;
}

void llg_value_scopes_end_since(llg_value_scope_t* mark) {
    llg_proc_t* owner = llg_current();
    llg_value_scope_t** head = owner ? &owner->value_scopes : &root_value_scopes;
    /* Validate first so a stale/foreign mark cannot partially unwind a caller. */
    if (mark) {
        llg_value_scope_t* cursor = *head;
        while (cursor && cursor != mark) cursor = cursor->next;
        if (!cursor) {
            fputs("llg runtime fatal: value scope mark is not registered\n", stderr);
            abort();
        }
    }
    while (*head != mark) llg_value_scope_end(*head);
}

static void value_scopes_unwind(llg_proc_t* proc) {
    llg_value_scope_t** head = proc ? &proc->value_scopes : &root_value_scopes;
    while (*head) llg_value_scope_end(*head);
}
