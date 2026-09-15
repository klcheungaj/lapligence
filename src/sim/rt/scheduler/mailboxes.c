
// ── Mailboxes ────────────────────────────────────────────────────────────────

static void mailbox_value_destroy(llg_mailbox_value_t* value) {
    if (!value) return;
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
    result.value.packed = value;
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
        sv4_t converted = sv4_cast(value->value.packed, target->width,
                                   target->is_signed);
        if (target->two_state) converted = sv4_to_two_state(converted);
        if (target->reference) llg_ref_write(target->reference, converted);
        else llg_ba(target->target.packed, converted);
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

static void mailbox_unlink_wait(llg_wait_t* wait) {
    if (!wait || !wait->mailbox) return;
    llg_mailbox_t* mailbox = wait->mailbox;
    llg_wait_t** head = wait->kind == W_MAILBOX_PUT
                            ? &mailbox->put_head
                            : &mailbox->get_head;
    llg_wait_t** tail = wait->kind == W_MAILBOX_PUT
                            ? &mailbox->put_tail
                            : &mailbox->get_tail;
    llg_wait_t** cursor = head;
    while (*cursor) {
        if (*cursor == wait) {
            *cursor = wait->mailbox_next;
            if (*tail == wait) *tail = NULL;
            if (!*head) {
                *tail = NULL;
            } else if (!*tail) {
                llg_wait_t* last = *head;
                while (last->mailbox_next) last = last->mailbox_next;
                *tail = last;
            }
            wait->mailbox_next = NULL;
            return;
        }
        cursor = &(*cursor)->mailbox_next;
    }
    wait->mailbox_next = NULL;
}

static void mailbox_append_wait(llg_mailbox_t* mailbox, llg_wait_t* wait,
                                 int put) {
    llg_wait_t** head = put ? &mailbox->put_head : &mailbox->get_head;
    llg_wait_t** tail = put ? &mailbox->put_tail : &mailbox->get_tail;
    wait->mailbox_next = NULL;
    if (*tail)
        (*tail)->mailbox_next = wait;
    else
        *head = wait;
    *tail = wait;
}

static void mailbox_type_error(void) {
    fprintf(stderr, "llg: mailbox retrieval type mismatch\n");
    llg_last_failure = 1;
    g.finish = 1;
    llg_proc_t* current = llg_current();
    if (current) llg_proc_done(current);
}

static void mailbox_remove_and_wake(llg_wait_t* wait) {
    if (!wait) return;
    mailbox_unlink_wait(wait);
    wait->mailbox = NULL;
    wake_proc(wait->proc);
}

// Service only FIFO heads. Unlinking/granting cannot execute a resumed
// continuation inline, so list ownership stays with this service loop.
static void mailbox_service_waiters(llg_mailbox_t* mailbox) {
    while (mailbox && !g.finish) {
        if (mailbox->head && mailbox->get_head) {
            llg_wait_t* get = mailbox->get_head;
            llg_mailbox_message_t* message = mailbox->head;
            if (!mailbox_target_matches(&message->value, &get->mailbox_target)) {
                mailbox_type_error();
                return;
            }
            mailbox_deliver(&message->value, &get->mailbox_target);
            if (!get->mailbox_peek) {
                message = mailbox_message_pop(mailbox);
                mailbox_value_destroy(&message->value);
                free(message);
            }
            mailbox_remove_and_wake(get);
            continue;
        }
        if (mailbox->put_head &&
            (mailbox->bound == 0 || mailbox->length < mailbox->bound)) {
            llg_wait_t* put = mailbox->put_head;
            llg_mailbox_value_t value = put->mailbox_value;
            memset(&put->mailbox_value, 0, sizeof(put->mailbox_value));
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

void llg_mailbox_put_value(llg_mailbox_t* mailbox, llg_mailbox_value_t value) {
    mailbox = mailbox_require(mailbox, "put");
    if (!mailbox) {
        mailbox_value_destroy(&value);
        return;
    }
    if (!mailbox_message_kind_matches(mailbox, &value)) {
        fprintf(stderr, "llg: mailbox put value does not match its type\n");
        llg_last_failure = 1;
        g.finish = 1;
        mailbox_value_destroy(&value);
        return;
    }
    if (mailbox->bound == 0 || mailbox->length < mailbox->bound) {
        mailbox_message_append(mailbox, value);
        mailbox_service_waiters(mailbox);
        return;
    }
    llg_proc_t* proc = llg_current();
    if (!proc || !region_can_mutate("mailbox put wait")) {
        mailbox_value_destroy(&value);
        return;
    }
    llg_wait_t* wait = &proc->wait;
    wait->kind = W_MAILBOX_PUT;
    wait->resume_region = region_is_reactive(proc->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    wait->mailbox = mailbox;
    wait->mailbox_value = value;
    register_wait();
    mailbox_append_wait(mailbox, wait, 1);
    aco_yield();
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
    llg_mailbox_message_t* message = mailbox->head;
    mailbox_deliver(&message->value, &target);
    if (!peek) {
        message = mailbox_message_pop(mailbox);
        mailbox_value_destroy(&message->value);
        free(message);
    }
    mailbox_service_waiters(mailbox);
    return 1;
}

static void llg_mailbox_wait_get(llg_mailbox_t* mailbox,
                                 llg_mailbox_target_t target, int peek) {
    llg_proc_t* proc = llg_current();
    if (!proc || !region_can_mutate("mailbox get wait")) return;
    llg_wait_t* wait = &proc->wait;
    wait->kind = W_MAILBOX_GET;
    wait->resume_region = region_is_reactive(proc->region)
                              ? LLG_REGION_REACTIVE
                              : LLG_REGION_ACTIVE;
    wait->mailbox = mailbox;
    wait->mailbox_target = target;
    wait->mailbox_peek = peek;
    register_wait();
    mailbox_append_wait(mailbox, wait, 0);
    aco_yield();
}

void llg_mailbox_get_value(llg_mailbox_t* mailbox, llg_mailbox_target_t target,
                           int peek) {
    mailbox = mailbox_require(mailbox, "get");
    if (!mailbox) return;
    int result = mailbox_take_value(mailbox, target, peek);
    if (result < 0) {
        mailbox_type_error();
        return;
    }
    if (result > 0) return;
    llg_mailbox_wait_get(mailbox, target, peek);
}

int llg_mailbox_try_get_value(llg_mailbox_t* mailbox,
                              llg_mailbox_target_t target, int peek) {
    mailbox = mailbox_require(mailbox, peek ? "try_peek" : "try_get");
    if (!mailbox) return 0;
    return mailbox_take_value(mailbox, target, peek);
}
