
static void activation_retain(llg_activation_t* activation) {
    if (!activation) return;
    if (activation->refs == SIZE_MAX) {
        llg_msg(LLG_MSG_INTERNAL, NULL, "named activation reference count overflow");
        abort();
    }
    activation->refs++;
}

static void activation_release(llg_activation_t* activation) {
    // Iterative: the parent chain follows lexical and call nesting, so its
    // length depends on the design and must not consume native stack.
    while (activation) {
        if (activation->refs == 0) {
            llg_msg(LLG_MSG_INTERNAL, NULL, "named activation reference count underflow");
            abort();
        }
        activation->refs--;
        if (activation->refs != 0) return;
        // A detached activation can remain the owner of a join_none group.
        // Keep its lexical ancestry alive until that last owner reference is
        // released so a later disable of an active outer scope still finds
        // the retained descendant through the parent chain.
        llg_activation_t* parent = activation->parent;
        activation->parent = NULL;
        free(activation);
        activation = parent;
    }
}

static void activation_unlink_all(llg_activation_t* activation) {
    if (!activation->all_prev_link) return;
    *activation->all_prev_link = activation->all_next;
    if (activation->all_next)
        activation->all_next->all_prev_link = activation->all_prev_link;
    activation->all_prev_link = NULL;
    activation->all_next = NULL;
}

static void activation_unlink_process(llg_activation_t* activation) {
    if (!activation->proc_prev_link) return;
    *activation->proc_prev_link = activation->proc_next;
    if (activation->proc_next)
        activation->proc_next->proc_prev_link = activation->proc_prev_link;
    activation->proc_prev_link = NULL;
    activation->proc_next = NULL;
}

static void activation_detach(llg_activation_t* activation) {
    if (!activation || activation->detached) return;
    activation_unlink_process(activation);
    activation_unlink_all(activation);
    activation->detached = 1;
}

typedef struct llg_ref_binding {
    llg_ref_t descriptor;
    void (*release)(void*);
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
    if (*top != scope) { llg_msg(LLG_MSG_INTERNAL, NULL, "unbalanced reference scopes"); abort(); }
    *top = scope->parent;
    while (scope->bindings) {
        llg_ref_binding_t* binding = scope->bindings;
        scope->bindings = binding->next;
        binding->release(binding->descriptor.retained);
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

// Bind one retained element cell in the innermost call scope, which releases
// it when the call completes or is unwound.
static llg_ref_t* reference_cell(uint32_t width, int8_t is_signed, uint8_t two_state,
                                 void* cell, sv4_t (*read)(const void*),
                                 int (*write)(void*, sv4_t), void (*release)(void*)) {
    llg_proc_t* proc = llg_current();
    llg_ref_scope_t* scope = proc ? proc->reference_top : root_reference_top;
    if (!scope) {
        release(cell);
        llg_msg(LLG_MSG_INTERNAL, NULL, "element reference without call scope");
        abort();
    }
    llg_ref_binding_t* binding = llg_checked_calloc(1, sizeof(*binding), "element reference");
    binding->descriptor.width = width;
    binding->descriptor.is_signed = is_signed;
    binding->descriptor.two_state = two_state;
    binding->descriptor.kind = LLG_REF_QUEUE;
    binding->descriptor.retained = cell;
    binding->descriptor.retained_read = read;
    binding->descriptor.retained_write = write;
    binding->release = release;
    binding->next = scope->bindings;
    scope->bindings = binding;
    return &binding->descriptor;
}

llg_ref_t* llg_ref_queue(llg_queue_t* queue, uint64_t index) {
    if (!queue) { llg_msg(LLG_MSG_INTERNAL, NULL, "queue reference without call scope"); abort(); }
    return reference_cell(queue->element_width, queue->element_signed,
                          queue->element_two_state, llg_queue_ref_acquire(queue, index),
                          llg_queue_cell_read, llg_queue_cell_write, llg_queue_ref_release);
}

llg_ref_t* llg_ref_dyn(llg_dyn_array_t* array, sv4_t index) {
    return reference_cell(array->element_width, array->element_signed,
                          array->element_two_state, llg_dyn_ref_acquire(array, index),
                          llg_element_cell_read, llg_element_cell_write,
                          llg_element_ref_release);
}

/* Binding a reference creates a missing entry, even for a `const ref`. */
llg_ref_t* llg_ref_assoc_integral(llg_assoc_t* array, sv4_t key) {
    if (!llg_assoc_exists_integral(array, key))
        llg_assoc_set_integral(array, key, array->default_value);
    return reference_cell(array->element_width, array->element_signed,
                          array->element_two_state,
                          llg_assoc_ref_acquire_integral(array, key),
                          llg_element_cell_read, llg_element_cell_write,
                          llg_element_ref_release);
}

llg_ref_t* llg_ref_assoc_string(llg_assoc_t* array, const void* key, size_t key_length) {
    if (!llg_assoc_exists_string(array, key, key_length))
        llg_assoc_set_string(array, key, key_length, array->default_value);
    return reference_cell(array->element_width, array->element_signed,
                          array->element_two_state,
                          llg_assoc_ref_acquire_string(array, key, key_length),
                          llg_element_cell_read, llg_element_cell_write,
                          llg_element_ref_release);
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
    activation->proc_prev_link = &proc->activation_top;
    if (activation->proc_next)
        activation->proc_next->proc_prev_link = &activation->proc_next;
    proc->activation_top = activation;
    activation->all_next = g.activations;
    activation->all_prev_link = &g.activations;
    if (activation->all_next)
        activation->all_next->all_prev_link = &activation->all_next;
    g.activations = activation;
    return activation;
}

void llg_activation_exit(llg_activation_t* activation) {
    if (!activation || activation->detached) return;
    activation_detach(activation);
    activation_release(activation);
}

void llg_activation_exit_terminal(llg_activation_t* activation) {
    if (activation && !activation->detached && activation->disabled && activation->proc)
        activation->proc->disabled_whole = 1;
    llg_activation_exit(activation);
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

int llg_rt_fatal_severity_ran(void) { return llg_fatal_severity_ran != 0; }

static void budget_abort(llg_proc_t* p, const char* location) {
    const char* where = location && location[0] != '\0'
                            ? location
                            : (p && p->name ? p->name : "<unknown process>");
    llg_msg(LLG_MSG_RUNTIME_ERROR, NULL, "nonconvergent zero-time execution in process `%s` at time %llu (process step limit %llu)", where, (unsigned long long)g.now, (unsigned long long)g.process_step_limit);
    llg_last_failure = 1;
    g.finish = 1;
    if (p) p->chain.exiting = LLG_EXIT_ABANDON;
}

int llg_index_eq(sv4_t index, int64_t key) {
    int64_t value;
    return sv4_to_index_i64(index, &value) && value == key;
}

int llg_index_lt(sv4_t index, int64_t key) {
    int64_t value;
    return sv4_to_index_i64(index, &value) && value < key;
}

int llg_budget_point(const char* location) {
    llg_proc_t* p = llg_current();
    llg_runtime_service_enter(p, "budget point");
    if (!p || g.config_error) return 0;
    if (p->budget_time != g.now) {
        p->budget_time = g.now;
        p->budget_steps = 0;
    }
    if (!consume_limit(&p->budget_steps, g.process_step_limit)) {
        budget_abort(p, location);
        return 1;
    }
    return 0;
}

void llg_unique_priority_check(int check, int matched, int has_default,
                               const char* location) {
    if (check < 1 || check > 3) return;
    const char* where = location && location[0] != '\0' ? location : "<unknown>";
    const char* qualifier = check == 1 ? "unique" : check == 2 ? "unique0" : "priority";
    if (matched == 0 && !has_default && check != 2) {
        llg_msg(LLG_MSG_RUNTIME_WARNING, NULL, "%s violation at %s: no matching item", qualifier, where);
    }
    if (matched > 1 && check != 3) {
        llg_msg(LLG_MSG_RUNTIME_WARNING, NULL, "%s violation at %s: multiple matching items", qualifier, where);
    }
}
