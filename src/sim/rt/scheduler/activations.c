
static void activation_retain(llg_activation_t* activation) {
    if (!activation) return;
    if (activation->refs == SIZE_MAX) {
        fprintf(stderr, "llg: named activation reference count overflow\n");
        abort();
    }
    activation->refs++;
}

static void activation_release(llg_activation_t* activation) {
    if (!activation) return;
    if (activation->refs == 0) {
        fprintf(stderr, "llg: named activation reference count underflow\n");
        abort();
    }
    activation->refs--;
    if (activation->refs == 0) {
        // A detached activation can remain the owner of a join_none group.
        // Keep its lexical ancestry alive until that last owner reference is
        // released so a later disable of an active outer scope still finds
        // the retained descendant through the parent chain.
        llg_activation_t* parent = activation->parent;
        activation->parent = NULL;
        free(activation);
        activation_release(parent);
    }
}

static void activation_unlink_all(llg_activation_t* activation) {
    llg_activation_t** pp = &g.activations;
    while (*pp) {
        if (*pp == activation) {
            *pp = activation->all_next;
            activation->all_next = NULL;
            return;
        }
        pp = &(*pp)->all_next;
    }
}

static void activation_unlink_process(llg_activation_t* activation) {
    llg_proc_t* proc = activation->proc;
    if (!proc) return;
    llg_activation_t** pp = &proc->activation_top;
    while (*pp) {
        if (*pp == activation) {
            *pp = activation->proc_next;
            activation->proc_next = NULL;
            return;
        }
        pp = &(*pp)->proc_next;
    }
}

static void activation_detach(llg_activation_t* activation) {
    if (!activation || activation->detached) return;
    activation_unlink_process(activation);
    activation_unlink_all(activation);
    activation->detached = 1;
}

typedef struct llg_ref_binding {
    llg_ref_t descriptor;
    struct llg_ref_binding* next;
} llg_ref_binding_t;

struct llg_ref_scope {
    llg_proc_t* proc;
    llg_ref_scope_t* parent;
    llg_ref_binding_t* bindings;
};

static llg_ref_scope_t* root_reference_top;

llg_ref_scope_t* llg_ref_scope_begin(void) {
    llg_proc_t* proc = llg_current();
    llg_ref_scope_t** top = proc ? &proc->reference_top : &root_reference_top;
    llg_ref_scope_t* scope = llg_checked_calloc(1, sizeof(*scope), "reference scope");
    scope->proc = proc;
    scope->parent = *top;
    *top = scope;
    return scope;
}

void llg_ref_scope_end(llg_ref_scope_t* scope) {
    if (!scope) return;
    llg_ref_scope_t** top = scope->proc ? &scope->proc->reference_top : &root_reference_top;
    if (*top != scope) { fprintf(stderr, "llg: unbalanced reference scopes\n"); abort(); }
    *top = scope->parent;
    while (scope->bindings) {
        llg_ref_binding_t* binding = scope->bindings;
        scope->bindings = binding->next;
        llg_queue_ref_release(binding->descriptor.retained);
        free(binding);
    }
    free(scope);
}

static void reference_owner_destroy(void* payload) {
    llg_ref_scope_t* scope = *(llg_ref_scope_t**)payload;
    if (scope) llg_ref_scope_end(scope);
}

void llg_ref_scope_begin_owned(void) {
    llg_value_scope_t* owner = llg_value_scope_begin_object(
        sizeof(llg_ref_scope_t*), reference_owner_destroy);
    *(llg_ref_scope_t**)llg_value_scope_object(owner) = llg_ref_scope_begin();
}

llg_ref_t* llg_ref_queue(llg_queue_t* queue, uint64_t index) {
    llg_proc_t* proc = llg_current();
    llg_ref_scope_t* scope = proc ? proc->reference_top : root_reference_top;
    if (!scope || !queue) { fprintf(stderr, "llg: queue reference without call scope\n"); abort(); }
    llg_ref_binding_t* binding = llg_checked_calloc(1, sizeof(*binding), "queue reference");
    binding->descriptor.width = queue->element_width;
    binding->descriptor.is_signed = queue->element_signed;
    binding->descriptor.two_state = queue->element_two_state;
    binding->descriptor.kind = LLG_REF_QUEUE;
    binding->descriptor.retained = llg_queue_ref_acquire(queue, index);
    binding->descriptor.retained_read = llg_queue_cell_read;
    binding->descriptor.retained_write = llg_queue_cell_write;
    binding->next = scope->bindings;
    scope->bindings = binding;
    return &binding->descriptor;
}

static void activation_unwind_proc(llg_proc_t* proc) {
    while (proc && proc->reference_top) llg_ref_scope_end(proc->reference_top);
    while (proc && proc->activation_top) {
        llg_activation_t* activation = proc->activation_top;
        activation_detach(activation);
        activation_release(activation);
    }
}

llg_activation_t* llg_activation_enter(uint32_t declaration,
                                        uint32_t instance) {
    llg_proc_t* proc = llg_current();
    if (!proc || !region_can_mutate("named activation scheduling")) return NULL;
    llg_activation_t* activation = (llg_activation_t*)llg_checked_calloc(
        1, sizeof(*activation), "named activation");
    activation->declaration = declaration;
    activation->instance = instance;
    activation->proc = proc;
    activation->parent = proc->activation_top;
    activation->refs = 1; // process activation-stack ownership
    if (activation->parent) activation_retain(activation->parent);
    activation->proc_next = proc->activation_top;
    proc->activation_top = activation;
    activation->all_next = g.activations;
    g.activations = activation;
    return activation;
}

void llg_activation_exit(llg_activation_t* activation) {
    if (!activation || activation->detached) return;
    activation_detach(activation);
    activation_release(activation);
}

int llg_activation_cancelled(void) {
    llg_proc_t* proc = llg_current();
    for (llg_activation_t* activation = proc ? proc->activation_top : NULL;
         activation; activation = activation->proc_next) {
        if (activation->disabled) return 1;
    }
    return 0;
}

int llg_rt_failed(void) { return llg_last_failure != 0; }

static void budget_abort(llg_proc_t* p, const char* location) {
    const char* where = location && location[0] != '\0'
                            ? location
                            : (p && p->name ? p->name : "<unknown process>");
    fprintf(stderr,
            "llg: nonconvergent zero-time execution in process `%s` at time "
            "%llu (process step limit %llu)\n",
            where, (unsigned long long)g.now,
            (unsigned long long)g.process_step_limit);
    llg_last_failure = 1;
    g.finish = 1;
    aco_exit();
    abort();
}

void llg_budget_point(const char* location) {
    llg_proc_t* p = llg_current();
    if (!p || g.config_error) return;
    if (p->budget_time != g.now) {
        p->budget_time = g.now;
        p->budget_steps = 0;
    }
    if (!consume_limit(&p->budget_steps, g.process_step_limit)) {
        budget_abort(p, location);
    }
}

void llg_unique_priority_check(int check, int matched, int has_default,
                               const char* location) {
    if (check < 1 || check > 3) return;
    const char* where = location && location[0] != '\0' ? location : "<unknown>";
    const char* qualifier = check == 1 ? "unique" : check == 2 ? "unique0" : "priority";
    if (matched == 0 && !has_default && check != 2) {
        fprintf(stderr,
                "llg: warning: %s violation at %s: no matching item\n",
                qualifier, where);
    }
    if (matched > 1 && check != 3) {
        fprintf(stderr,
                "llg: warning: %s violation at %s: multiple matching items\n",
                qualifier, where);
    }
}
