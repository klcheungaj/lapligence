
// ── Mailboxes ────────────────────────────────────────────────────────────────

static void mailbox_value_destroy(llg_mailbox_value_t* value) {
    if (!value) return;
    if (value->kind == LLG_MAILBOX_PACKED) sv4_destroy(&value->value.packed);
    if (value->kind == LLG_MAILBOX_STRING)
        llg_string_destroy(&value->value.string);
    memset(value, 0, sizeof(*value));
}

llg_mailbox_value_t llg_mailbox_typed_value(llg_mailbox_value_t value, uint64_t type_id) {
    value.type_id = type_id;
    return value;
}

llg_mailbox_target_t llg_mailbox_typed_target(llg_mailbox_target_t target, uint64_t type_id) {
    target.type_id = type_id;
    return target;
}

llg_mailbox_target_t llg_mailbox_target_ref(llg_ref_t* ref) {
    llg_mailbox_target_t target = {0};
    target.kind = LLG_MAILBOX_PACKED;
    target.reference = ref;
    if (ref) {
        target.width = ref->width;
        target.is_signed = ref->is_signed;
        target.two_state = ref->two_state;
    }
    return target;
}

llg_mailbox_value_t llg_mailbox_value_packed(sv4_t value, uint32_t width,
                                              int is_signed, int two_state) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_PACKED;
    result.width = width;
    result.is_signed = (int8_t)is_signed;
    result.two_state = (int8_t)two_state;
    result.value.packed = sv4_clone(&value);
    return result;
}

llg_mailbox_value_t llg_mailbox_value_real(double value, int shortreal) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_REAL;
    result.shortreal = (int8_t)shortreal;
    result.value.real = shortreal ? (double)(float)value : value;
    return result;
}

llg_mailbox_value_t llg_mailbox_value_string(llg_string_t value) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_STRING;
    result.value.string = value;
    return result;
}

llg_mailbox_value_t llg_mailbox_value_handle(void* value) {
    llg_mailbox_value_t result = {0};
    result.kind = LLG_MAILBOX_HANDLE;
    result.value.handle = value;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_packed(sv4_t* target, uint32_t width,
                                                int is_signed, int two_state) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_PACKED;
    result.width = width;
    result.is_signed = (int8_t)is_signed;
    result.two_state = (int8_t)two_state;
    result.target.packed = target;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_real(double* target, int shortreal) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_REAL;
    result.shortreal = (int8_t)shortreal;
    result.target.real = target;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_string(llg_string_t* target) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_STRING;
    result.target.string = target;
    return result;
}

llg_mailbox_target_t llg_mailbox_target_handle(void** target) {
    llg_mailbox_target_t result = {0};
    result.kind = LLG_MAILBOX_HANDLE;
    result.target.handle = target;
    return result;
}

static int mailbox_message_kind_matches(const llg_mailbox_t* mailbox,
                                        const llg_mailbox_value_t* value) {
    if (!mailbox || !value) return 0;
    if (mailbox->kind == LLG_MAILBOX_UNTYPED) return 1;
    if (mailbox->kind != value->kind) return 0;
    switch (mailbox->kind) {
    case LLG_MAILBOX_PACKED:
        return mailbox->width == value->width &&
               mailbox->is_signed == value->is_signed &&
               mailbox->two_state == value->two_state;
    case LLG_MAILBOX_REAL:
        return mailbox->shortreal == value->shortreal;
    case LLG_MAILBOX_STRING:
    case LLG_MAILBOX_HANDLE:
        return 1;
    default:
        return 0;
    }
}

static int mailbox_target_matches(const llg_mailbox_value_t* value,
                                  const llg_mailbox_target_t* target) {
    if (!value || !target || value->kind != target->kind ||
        value->type_id != target->type_id) return 0;
    switch (value->kind) {
    case LLG_MAILBOX_PACKED:
        return value->width == target->width &&
               value->is_signed == target->is_signed &&
               value->two_state == target->two_state;
    case LLG_MAILBOX_REAL:
        return value->shortreal == target->shortreal;
    case LLG_MAILBOX_STRING:
        return 1;
    case LLG_MAILBOX_HANDLE:
        // The declared nominal type was checked above, even for null values.
        return 1;
    default:
        return 0;
    }
}

static void mailbox_deliver(const llg_mailbox_value_t* value,
                            const llg_mailbox_target_t* target) {
    if (!mailbox_target_matches(value, target)) return;
    if (!target->target.packed && !target->reference && target->kind == LLG_MAILBOX_PACKED) return;
    if (!target->target.real && target->kind == LLG_MAILBOX_REAL) return;
    if (!target->target.string && target->kind == LLG_MAILBOX_STRING) return;
    if (!target->target.handle && target->kind == LLG_MAILBOX_HANDLE) return;
    switch (target->kind) {
    case LLG_MAILBOX_PACKED: {
        llg_value_scope_t* scope = llg_value_scope_begin(1);
        sv4_t* converted = llg_value_scope_values(scope);
        sv4_replace(converted, sv4_cast(value->value.packed, target->width, target->is_signed));
        if (target->two_state) sv4_replace(converted, sv4_to_two_state(*converted));
        if (target->reference) llg_ref_write(target->reference, *converted);
        else llg_ba(target->target.packed, *converted);
        llg_value_scope_end(scope);
        break;
    }
    case LLG_MAILBOX_REAL:
        llg_ba_d(target->target.real,
                 target->shortreal ? (double)(float)value->value.real
                                   : value->value.real);
        break;
    case LLG_MAILBOX_STRING:
        llg_string_move(target->target.string,
                        llg_string_clone(&value->value.string));
        break;
    case LLG_MAILBOX_HANDLE:
        *target->target.handle = value->value.handle;
        break;
    default:
        break;
    }
}

static void mailbox_message_append(llg_mailbox_t* mailbox,
                                   llg_mailbox_value_t value) {
    llg_mailbox_message_t* message = (llg_mailbox_message_t*)llg_checked_calloc(
        1, sizeof(*message), "mailbox message");
    message->value = value;
    if (mailbox->tail)
        mailbox->tail->next = message;
    else
        mailbox->head = message;
    mailbox->tail = message;
    mailbox->length++;
}

static llg_mailbox_message_t* mailbox_message_pop(llg_mailbox_t* mailbox) {
    llg_mailbox_message_t* message = mailbox->head;
    if (!message) return NULL;
    mailbox->head = message->next;
    if (!mailbox->head) mailbox->tail = NULL;
    message->next = NULL;
    mailbox->length--;
    return message;
}

static void mailbox_snapshot_destroy(void* payload) {
    mailbox_value_destroy((llg_mailbox_value_t*)payload);
}

/* Freeze a successful delivery before publishing to HDL. A peek needs an
 * independent copy because a reentrant get can destroy the queue's head. */
static llg_value_scope_t* mailbox_snapshot(llg_mailbox_t* mailbox, int peek) {
    llg_value_scope_t* owner = llg_value_scope_begin_object(
        sizeof(llg_mailbox_value_t), mailbox_snapshot_destroy);
    llg_mailbox_value_t* value = llg_value_scope_object(owner);
    llg_mailbox_message_t* message = peek ? mailbox->head : mailbox_message_pop(mailbox);
    *value = message->value;
    if (peek) {
        if (value->kind == LLG_MAILBOX_PACKED)
            value->value.packed = sv4_clone(&message->value.value.packed);
        else if (value->kind == LLG_MAILBOX_STRING)
            value->value.string = llg_string_clone(&message->value.value.string);
    } else {
        memset(&message->value, 0, sizeof(message->value));
        free(message);
    }
    return owner;
}

static void mailbox_unlink_wait(llg_wait_t* wait) {
    if (!wait || !wait->payload.rare) return;
    llg_mailbox_t* mailbox = wait->kind == W_MAILBOX_PUT
                                 ? wait->payload.rare->mailbox_put.mailbox
                                 : wait->payload.rare->mailbox_get.mailbox;
    if (!mailbox) return;
    llg_wait_t** head = wait->kind == W_MAILBOX_PUT
                            ? &mailbox->put_head
                            : &mailbox->get_head;
    llg_wait_t** tail = wait->kind == W_MAILBOX_PUT
                            ? &mailbox->put_tail
                            : &mailbox->get_tail;
    llg_wait_t** cursor = head;
    while (*cursor) {
        if (*cursor == wait) {
            *cursor = wait->kind == W_MAILBOX_PUT
                          ? wait->payload.rare->mailbox_put.next
                          : wait->payload.rare->mailbox_get.next;
            if (*tail == wait) *tail = NULL;
            if (!*head) {
                *tail = NULL;
            } else if (!*tail) {
                llg_wait_t* last = *head;
                if (wait->kind == W_MAILBOX_PUT) {
                    while (last->payload.rare->mailbox_put.next)
                        last = last->payload.rare->mailbox_put.next;
                } else {
                    while (last->payload.rare->mailbox_get.next)
                        last = last->payload.rare->mailbox_get.next;
                }
                *tail = last;
            }
            if (wait->kind == W_MAILBOX_PUT)
                wait->payload.rare->mailbox_put.next = NULL;
            else
                wait->payload.rare->mailbox_get.next = NULL;
            return;
        }
        cursor = wait->kind == W_MAILBOX_PUT
                     ? &(*cursor)->payload.rare->mailbox_put.next
                     : &(*cursor)->payload.rare->mailbox_get.next;
    }
    if (wait->kind == W_MAILBOX_PUT)
        wait->payload.rare->mailbox_put.next = NULL;
    else
        wait->payload.rare->mailbox_get.next = NULL;
}

static void mailbox_append_wait(llg_mailbox_t* mailbox, llg_wait_t* wait,
                                 int put) {
    llg_wait_t** head = put ? &mailbox->put_head : &mailbox->get_head;
    llg_wait_t** tail = put ? &mailbox->put_tail : &mailbox->get_tail;
    llg_wait_t** next = put ? &wait->payload.rare->mailbox_put.next
                            : &wait->payload.rare->mailbox_get.next;
    *next = NULL;
    if (*tail) {
        if (put)
            (*tail)->payload.rare->mailbox_put.next = wait;
        else
            (*tail)->payload.rare->mailbox_get.next = wait;
    } else {
        *head = wait;
    }
    *tail = wait;
}

static void mailbox_type_error(void) {
    fprintf(stderr, "llg: mailbox retrieval type mismatch\n");
    llg_last_failure = 1;
    g.finish = 1;
    llg_proc_t* current = llg_current();
    if (current) current->chain.exiting = LLG_EXIT_COMPLETE;
}

static void mailbox_remove_and_wake(llg_wait_t* wait) {
    if (!wait) return;
    mailbox_unlink_wait(wait);
    if (wait->kind == W_MAILBOX_PUT)
        wait->payload.rare->mailbox_put.mailbox = NULL;
    else
        wait->payload.rare->mailbox_get.mailbox = NULL;
    wake_proc(wait->proc);
}

// Service only FIFO heads. Unlinking/granting cannot execute a resumed
// continuation inline, so list ownership stays with this service loop.
static void mailbox_service_waiters(llg_mailbox_t* mailbox) {
    while (mailbox && !g.finish) {
        if (mailbox->head && mailbox->get_head) {
            llg_wait_t* get = mailbox->get_head;
            llg_wait_mailbox_get_payload_t* payload =
                &get->payload.rare->mailbox_get;
            llg_mailbox_message_t* message = mailbox->head;
            if (!mailbox_target_matches(&message->value, &payload->target)) {
                mailbox_type_error();
                return;
            }
            llg_mailbox_target_t target = payload->target;
            llg_value_scope_t* owner = mailbox_snapshot(mailbox, payload->peek);
            /* wake_proc queues, but never runs, the continuation. Copy the
             * destination before wakeup clears the wait record. */
            mailbox_remove_and_wake(get);
            mailbox_deliver(llg_value_scope_object(owner), &target);
            llg_value_scope_end(owner);
            continue;
        }
        if (mailbox->put_head &&
            (mailbox->bound == 0 || mailbox->length < mailbox->bound)) {
            llg_wait_t* put = mailbox->put_head;
            llg_mailbox_value_t* stored = &put->payload.rare->mailbox_put.value;
            llg_mailbox_value_t value = *stored;
            memset(stored, 0, sizeof(*stored));
            mailbox_message_append(mailbox, value);
            mailbox_remove_and_wake(put);
            continue;
        }
        break;
    }
}

static llg_mailbox_t* mailbox_require(llg_mailbox_t* mailbox,
                                      const char* operation) {
    if (mailbox) return mailbox;
    fprintf(stderr, "llg: mailbox %s on a null handle\n", operation);
    llg_last_failure = 1;
    g.finish = 1;
    return NULL;
}

llg_mailbox_t* llg_mailbox_new(sv4_t bound, int kind, uint32_t width,
                               int is_signed, int two_state, int shortreal) {
    if (sv4_is_unknown(bound)) {
        fprintf(stderr, "llg: mailbox bound contains X/Z\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    if (bound.width > 64) {
        fprintf(stderr, "llg: mailbox bound exceeds 64-bit capacity\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    if (bound.is_signed && sv4_to_i64(bound) < 0) {
        fprintf(stderr, "llg: mailbox bound must be non-negative\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    if (kind < LLG_MAILBOX_PACKED || kind > LLG_MAILBOX_UNTYPED ||
        (kind == LLG_MAILBOX_PACKED && width == 0) ||
        (kind != LLG_MAILBOX_PACKED && width != 0) ||
        (kind != LLG_MAILBOX_PACKED && (is_signed || two_state)) ||
        (kind != LLG_MAILBOX_REAL && shortreal)) {
        fprintf(stderr, "llg: invalid mailbox element descriptor\n");
        llg_last_failure = 1;
        g.finish = 1;
        return NULL;
    }
    llg_mailbox_t* mailbox = (llg_mailbox_t*)llg_checked_calloc(
        1, sizeof(*mailbox), "mailbox");
    mailbox->bound = sv4_to_u64(bound);
    mailbox->kind = kind;
    mailbox->width = width;
    mailbox->is_signed = (int8_t)is_signed;
    mailbox->two_state = (int8_t)two_state;
    mailbox->shortreal = (int8_t)shortreal;
    mailbox->next = g.mailboxes;
    g.mailboxes = mailbox;
    return mailbox;
}

uint64_t llg_mailbox_num(const llg_mailbox_t* mailbox) {
    mailbox = mailbox_require((llg_mailbox_t*)mailbox, "num");
    return mailbox ? mailbox->length : 0;
}

llg_co_arm_t llg_arm_mailbox_put_value(llg_proc_t* self,
                                       llg_mailbox_t* mailbox,
                                       llg_mailbox_value_t value) {
    llg_runtime_service_enter(self, "mailbox::put");
    mailbox = mailbox_require(mailbox, "put");
    if (!mailbox) {
        mailbox_value_destroy(&value);
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    if (!mailbox_message_kind_matches(mailbox, &value)) {
        fprintf(stderr, "llg: mailbox put value does not match its type\n");
        llg_last_failure = 1;
        g.finish = 1;
        mailbox_value_destroy(&value);
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    if (mailbox->bound == 0 || mailbox->length < mailbox->bound) {
        mailbox_message_append(mailbox, value);
        mailbox_service_waiters(mailbox);
        return self && self->chain.exiting ? LLG_CO_ARM_EXIT
                                           : LLG_CO_ARM_READY;
    }
    if (!self || !region_can_mutate("mailbox put wait")) {
        mailbox_value_destroy(&value);
        return LLG_CO_ARM_READY;
    }
    llg_wait_t* wait = &self->wait;
    wait->kind = W_MAILBOX_PUT;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    llg_wait_mailbox_put_payload_t* payload =
        &wait_rare_allocate(wait, "mailbox put wait payload")->mailbox_put;
    payload->mailbox = mailbox;
    payload->value = value;
    register_wait();
    mailbox_append_wait(mailbox, wait, 1);
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

int llg_mailbox_try_put_value(llg_mailbox_t* mailbox,
                              llg_mailbox_value_t value) {
    mailbox = mailbox_require(mailbox, "try_put");
    if (!mailbox) {
        mailbox_value_destroy(&value);
        return 0;
    }
    if (!mailbox_message_kind_matches(mailbox, &value)) {
        mailbox_value_destroy(&value);
        return 0;
    }
    if (mailbox->bound != 0 && mailbox->length >= mailbox->bound) {
        mailbox_value_destroy(&value);
        return 0;
    }
    mailbox_message_append(mailbox, value);
    mailbox_service_waiters(mailbox);
    return 1;
}

// 0 is empty, -1 is a type mismatch, and 1 is a successful transfer.
// A failed conversion never changes either the message or the destination.
static int mailbox_take_value(llg_mailbox_t* mailbox,
                              llg_mailbox_target_t target, int peek) {
    if (!mailbox || !mailbox->head) return 0;
    if (!mailbox_target_matches(&mailbox->head->value, &target)) return -1;
    llg_value_scope_t* owner = mailbox_snapshot(mailbox, peek);
    mailbox_deliver(llg_value_scope_object(owner), &target);
    llg_value_scope_end(owner);
    mailbox_service_waiters(mailbox);
    return 1;
}

static llg_co_arm_t llg_mailbox_wait_get(llg_proc_t* self,
                                         llg_mailbox_t* mailbox,
                                         llg_mailbox_target_t target,
                                         int peek) {
    if (!self || !region_can_mutate("mailbox get wait"))
        return LLG_CO_ARM_READY;
    llg_wait_t* wait = &self->wait;
    wait->kind = W_MAILBOX_GET;
    wait->resume_region = region_is_reactive(self->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    llg_wait_mailbox_get_payload_t* payload =
        &wait_rare_allocate(wait, "mailbox get wait payload")->mailbox_get;
    payload->mailbox = mailbox;
    payload->target = target;
    payload->peek = peek;
    register_wait();
    mailbox_append_wait(mailbox, wait, 0);
    start_pending_fork_children(self);
    return LLG_CO_ARM_SUSPEND;
}

llg_co_arm_t llg_arm_mailbox_get_value(llg_proc_t* self,
                                       llg_mailbox_t* mailbox,
                                       llg_mailbox_target_t target, int peek) {
    llg_runtime_service_enter(self, "mailbox::get");
    mailbox = mailbox_require(mailbox, "get");
    if (!mailbox) {
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    int result = mailbox_take_value(mailbox, target, peek);
    if (result < 0) {
        mailbox_type_error();
        if (self) self->chain.exiting = LLG_EXIT_COMPLETE;
        return LLG_CO_ARM_EXIT;
    }
    if (result > 0)
        return self && self->chain.exiting ? LLG_CO_ARM_EXIT
                                           : LLG_CO_ARM_READY;
    return llg_mailbox_wait_get(self, mailbox, target, peek);
}

int llg_mailbox_try_get_value(llg_mailbox_t* mailbox,
                              llg_mailbox_target_t target, int peek) {
    mailbox = mailbox_require(mailbox, peek ? "try_peek" : "try_get");
    if (!mailbox) return 0;
    return mailbox_take_value(mailbox, target, peek);
}
