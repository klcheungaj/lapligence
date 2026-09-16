
llg_value_scope_t* llg_value_scope_begin(size_t count) {
    llg_value_scope_t* scope = (llg_value_scope_t*)llg_checked_calloc(
        1, sizeof(*scope), "value owner scope");
    scope->values = (sv4_t*)llg_checked_calloc(count, sizeof(sv4_t), "scoped values");
    scope->count = count;
    scope->owner = llg_current();
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    scope->next = *head;
    *head = scope;
    return scope;
}

sv4_t* llg_value_scope_values(llg_value_scope_t* scope) {
    return scope ? scope->values : NULL;
}

void llg_value_scope_end(llg_value_scope_t* scope) {
    if (!scope) return;
    llg_value_scope_t** head = scope->owner ? &scope->owner->value_scopes : &root_value_scopes;
    while (*head && *head != scope) head = &(*head)->next;
    if (!*head) {
        fputs("llg runtime fatal: value scope is not registered\n", stderr);
        abort();
    }
    *head = scope->next;
    sv4_destroy_array(scope->values, scope->count);
    free(scope->values);
    free(scope);
}

static void value_scopes_unwind(llg_proc_t* proc) {
    llg_value_scope_t** head = proc ? &proc->value_scopes : &root_value_scopes;
    while (*head) llg_value_scope_end(*head);
}
