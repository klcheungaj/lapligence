
static void llg_file_set_message(char* target, size_t capacity, const char* message) {
    if (!capacity) return;
    const char* source = message ? message : "";
    size_t length = 0;
    while (length < capacity - 1u && source[length]) ++length;
    // Diagnostic text may be reused or shortened in place. Bound the read and
    // accept overlap without strncpy's implicit padding/truncation contract.
    memmove(target, source, length);
    target[length] = 0;
}

static void llg_file_global_failure(const char* message) {
    llg_file_global_error = 1;
    llg_file_set_message(llg_file_global_message, sizeof(llg_file_global_message), message);
}

static void llg_file_slot_failure(llg_file_slot_t* slot, const char* message) {
    slot->error = 1;
    llg_file_set_message(slot->message, sizeof(slot->message), message);
}

static void llg_file_init_table(void) {
    if (llg_files_initialized) return;
    memset(llg_file_slots, 0, sizeof(llg_file_slots));
    llg_file_slots[LLG_FILE_STDIN].stream = stdin;
    llg_file_slots[LLG_FILE_STDOUT].stream = stdout;
    llg_file_slots[LLG_FILE_STDERR].stream = stderr;
    for (unsigned i = 0; i < 3u; ++i) llg_file_slots[i].open = 1;
    llg_files_initialized = 1;
    llg_file_global_error = 0;
    llg_file_global_message[0] = 0;
}

static uint32_t llg_file_mcd_bit(unsigned slot) {
    if (slot == LLG_FILE_STDOUT) return 1u;
    if (slot >= LLG_FILE_MCD_FIRST && slot < LLG_FILE_MCD_END)
        return UINT32_C(1) << (slot - 2u);
    return 0;
}

static int llg_file_selected(uint32_t descriptor, unsigned slot) {
    if (descriptor & LLG_FILE_FD_TAG)
        return (descriptor & ~LLG_FILE_FD_TAG) == slot;
    return (descriptor & llg_file_mcd_bit(slot)) != 0;
}

static int llg_file_mask_valid(uint32_t descriptor) {
    llg_file_init_table();
    if (!descriptor) {
        llg_file_global_failure("invalid file descriptor");
        return 0;
    }
    if (descriptor & LLG_FILE_FD_TAG) {
        uint32_t index = descriptor & ~LLG_FILE_FD_TAG;
        if (index >= LLG_FILE_SLOTS ||
            (index >= 3u && index < LLG_FILE_FD_FIRST) ||
            !llg_file_slots[index].open || !llg_file_slots[index].stream) {
            llg_file_global_failure("invalid or closed file descriptor");
            return 0;
        }
        return 1;
    }
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (llg_file_selected(descriptor, i) &&
            (!llg_file_slots[i].open || !llg_file_slots[i].stream)) {
            llg_file_global_failure("invalid or closed multichannel descriptor");
            return 0;
        }
    }
    return 1;
}

static int llg_file_single_ordinary(uint32_t descriptor, llg_file_slot_t** out) {
    if (!(descriptor & LLG_FILE_FD_TAG) || !llg_file_mask_valid(descriptor)) {
        llg_file_global_failure("file input/position operation requires an FD, not an MCD");
        return 0;
    }
    *out = &llg_file_slots[descriptor & ~LLG_FILE_FD_TAG];
    return 1;
}

uint32_t llg_file_descriptor(sv4_t value) {
    // A descriptor is a 32-bit bit pattern, not a nonnegative signed integer.
    if (value.width == 0 || value.width > 32 || sv4_is_unknown(value)) {
        llg_file_global_failure("file descriptor is not a known 32-bit value");
        return 0;
    }
    uint32_t descriptor = (uint32_t)sv4_to_u64(value);
    if (descriptor == 0) llg_file_global_failure("invalid file descriptor");
    return descriptor;
}

uint32_t llg_file_open(llg_string_t path, llg_string_t mode, int has_mode) {
    llg_file_init_table();
    const char* selected_mode = has_mode ? mode.data : "w";
    size_t mode_len = has_mode ? mode.len : 1u;
    const char* selected_path = path.data ? path.data : "";
    char* path_copy = (char*)llg_checked_malloc(path.len + 1u, 1, "file path");
    memcpy(path_copy, selected_path, path.len);
    path_copy[path.len] = 0;
    char* mode_copy = (char*)llg_checked_malloc(mode_len + 1u, 1, "file mode");
    memcpy(mode_copy, selected_mode ? selected_mode : "", mode_len);
    mode_copy[mode_len] = 0;
    llg_string_destroy(&path);
    llg_string_destroy(&mode);

    unsigned slot_index = LLG_FILE_SLOTS;
    for (unsigned i = has_mode ? LLG_FILE_FD_FIRST : LLG_FILE_MCD_FIRST;
         i < (has_mode ? LLG_FILE_SLOTS : LLG_FILE_MCD_END); i++) {
        if (!llg_file_slots[i].open) {
            slot_index = i;
            break;
        }
    }
    if (slot_index == LLG_FILE_SLOTS) {
        llg_file_global_failure("file descriptor table is full");
        free(path_copy);
        free(mode_copy);
        return 0;
    }

    static const char* const valid_modes[] = {
        "r", "w", "a", "r+", "w+", "a+",
        "rb", "wb", "ab", "r+b", "w+b", "a+b", "rb+", "wb+", "ab+",
    };
    int mode_valid = 0;
    for (size_t i = 0; i < sizeof(valid_modes) / sizeof(valid_modes[0]); i++) {
        if (strcmp(mode_copy, valid_modes[i]) == 0) {
            mode_valid = 1;
            break;
        }
    }
    if (!mode_valid) {
        llg_file_global_failure("unsupported file open mode");
        free(path_copy);
        free(mode_copy);
        return 0;
    }
    FILE* stream = fopen(path_copy, mode_copy);
    if (!stream) {
        char message[160];
        snprintf(message, sizeof(message), "file open failed: %s", strerror(errno));
        llg_file_global_failure(message);
        free(path_copy);
        free(mode_copy);
        return 0;
    }
    free(path_copy);
    free(mode_copy);
    llg_file_slots[slot_index].stream = stream;
    llg_file_slots[slot_index].open = 1;
    llg_file_slots[slot_index].owned = 1;
    llg_file_slots[slot_index].error = 0;
    llg_file_slots[slot_index].eof = 0;
    llg_file_slots[slot_index].pushback_len = 0;
    llg_file_slots[slot_index].message[0] = 0;
    return has_mode ? LLG_FILE_FD_TAG | slot_index : llg_file_mcd_bit(slot_index);
}

// Cancel deferred output before the slot can be reused by a later fopen.
static uint32_t llg_file_without_slot(uint32_t descriptor, unsigned slot) {
    if (descriptor & LLG_FILE_FD_TAG)
        return llg_file_selected(descriptor, slot) ? 0 : descriptor;
    return descriptor & ~llg_file_mcd_bit(slot);
}

void llg_file_close(uint32_t descriptor) {
    if (!llg_file_mask_valid(descriptor)) return;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (!llg_file_selected(descriptor, i)) continue;
        if (g.mon.typed) g.mon.descriptor = llg_file_without_slot(g.mon.descriptor, i);
        for (llg_strobe_t* e = g.strobes; e; e = e->next)
            if (e->typed) e->descriptor = llg_file_without_slot(e->descriptor, i);
        llg_file_slot_t* slot = &llg_file_slots[i];
        // Preopened streams are borrowed from the host. Invalidate their HDL
        // descriptors without closing the host's diagnostic/output channel.
        int result = slot->owned ? fclose(slot->stream) : 0;
        slot->stream = NULL;
        slot->open = 0;
        slot->owned = 0;
        slot->pushback_len = 0;
        if (result != 0) llg_file_slot_failure(slot, "file close failed");
    }
}

int llg_file_flush(uint32_t descriptor, int all) {
    llg_file_init_table();
    if (!all && !llg_file_mask_valid(descriptor)) return -1;
    int result = 0;
    for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
        if (all ? (!llg_file_slots[i].open || i == LLG_FILE_STDIN)
                : !llg_file_selected(descriptor, i)) continue;
        if (!llg_file_slots[i].open || !llg_file_slots[i].stream) {
            llg_file_global_failure("invalid or closed file descriptor");
            result = -1;
            continue;
        }
        if (fflush(llg_file_slots[i].stream) != 0) {
            llg_file_slot_failure(&llg_file_slots[i], "file flush failed");
            result = -1;
        }
    }
    return result;
}

void llg_file_rewind(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return;
    rewind(slot->stream);
    slot->error = 0;
    slot->eof = 0;
    slot->pushback_len = 0;
    slot->message[0] = 0;
}

int64_t llg_file_tell(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return -1;
    long position = ftell(slot->stream);
    if (position < 0) {
        llg_file_slot_failure(slot, "file tell failed");
        return -1;
    }
    if ((uint64_t)position < slot->pushback_len) {
        llg_file_slot_failure(slot, "file tell position precedes pushed-back characters");
        return -1;
    }
    return (int64_t)position - (int64_t)slot->pushback_len;
}

int llg_file_seek(uint32_t descriptor, sv4_t offset, sv4_t operation) {
    llg_file_slot_t* slot;
    int64_t signed_offset;
    if (!llg_file_single_ordinary(descriptor, &slot) ||
        !sv4_to_index_i64(offset, &signed_offset) || sv4_is_unknown(operation) ||
        operation.width == 0 || sv4_to_u64(operation) > 2u) {
        llg_file_global_failure("invalid file seek arguments");
        return -1;
    }
    int whence = (int)sv4_to_u64(operation);
    if (whence == SEEK_CUR) {
        if (signed_offset < INT64_MIN + (int64_t)slot->pushback_len) {
            llg_file_slot_failure(slot, "file seek offset underflow");
            return -1;
        }
        signed_offset -= (int64_t)slot->pushback_len;
    }
    if (signed_offset < (int64_t)LONG_MIN || signed_offset > (int64_t)LONG_MAX ||
        fseek(slot->stream, (long)signed_offset, whence) != 0) {
        llg_file_slot_failure(slot, "file seek failed");
        return -1;
    }
    slot->eof = 0;
    slot->pushback_len = 0;
    return 0;
}

int llg_file_error(uint32_t descriptor, llg_string_t* message) {
    const char* text = "";
    int result = 0;
    if (!llg_file_mask_valid(descriptor)) {
        result = 1;
        text = llg_file_global_message;
    } else {
        for (unsigned i = 0; i < LLG_FILE_SLOTS; i++) {
            if (!llg_file_selected(descriptor, i)) continue;
            llg_file_slot_t* slot = &llg_file_slots[i];
            if (ferror(slot->stream)) llg_file_slot_failure(slot, "host stream error");
            if (slot->error) {
                result = 1;
                text = slot->message;
                break;
            }
        }
    }
    if (message) llg_string_move(message, llg_string_bytes(text, strlen(text)));
    return result;
}

int llg_file_eof(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return -1;
    return slot->eof != 0;
}

// ── File input ──────────────────────────────────────────────────────────────

static int llg_file_getc_slot(llg_file_slot_t* slot) {
    if (!slot || !slot->open || !slot->stream) return EOF;
    int value;
    if (slot->pushback_len) {
        value = slot->pushback[--slot->pushback_len];
        slot->eof = 0;
        return value;
    }
    value = fgetc(slot->stream);
    if (value == EOF) {
        if (feof(slot->stream)) slot->eof = 1;
        if (ferror(slot->stream)) llg_file_slot_failure(slot, "file input failed");
    } else {
        slot->eof = 0;
    }
    return value;
}

static int llg_file_ungetc_slot(llg_file_slot_t* slot, int value) {
    if (!slot || !slot->open || !slot->stream || value == EOF || value < 0 || value > UCHAR_MAX)
        return EOF;
    if (slot->pushback_len >= LLG_FILE_PUSHBACK) {
        llg_file_slot_failure(slot, "file input pushback limit exceeded");
        return EOF;
    }
    slot->pushback[slot->pushback_len++] = (unsigned char)value;
    // A successful standard-library ungetc clears the stream EOF indicator;
    // mirror that behavior even though the bounded stack keeps bytes outside
    // the host FILE buffer.
    clearerr(slot->stream);
    slot->eof = 0;
    return value;
}

int llg_file_getc(uint32_t descriptor) {
    llg_file_slot_t* slot;
    if (!llg_file_single_ordinary(descriptor, &slot)) return EOF;
    return llg_file_getc_slot(slot);
}

int llg_file_ungetc(uint32_t descriptor, sv4_t character) {
    llg_file_slot_t* slot;
    int64_t value;
    if (!llg_file_single_ordinary(descriptor, &slot) ||
        !sv4_to_index_i64(character, &value) || value < 0 || value > UCHAR_MAX) {
        llg_file_global_failure("invalid file ungetc arguments");
        return EOF;
    }
    return llg_file_ungetc_slot(slot, (int)value);
}

int llg_file_gets(uint32_t descriptor, llg_string_t* target) {
    llg_file_slot_t* slot;
    if (!target || !llg_file_single_ordinary(descriptor, &slot)) return 0;
    size_t capacity = 128u;
    size_t length = 0;
    unsigned char* bytes = (unsigned char*)llg_checked_malloc(capacity, 1, "file input line");
    for (;;) {
        int value = llg_file_getc_slot(slot);
        if (value == EOF) break;
        if (length == capacity) {
            if (capacity > (SIZE_MAX / 2u)) {
                free(bytes);
                llg_fatal_allocation("file input line", capacity, 2u);
            }
            capacity *= 2u;
            unsigned char* replacement = (unsigned char*)realloc(bytes, capacity);
            if (!replacement) {
                free(bytes);
                llg_fatal_allocation("file input line", capacity, 1u);
            }
            bytes = replacement;
        }
        bytes[length++] = (unsigned char)value;
        if (value == '\n') break;
    }
    if (length == 0) {
        free(bytes);
        return 0;
    }
    llg_string_t line = llg_string_bytes((const char*)bytes, length);
    free(bytes);
    /* Commit owns the string before it notifies; no line buffer remains on
     * the abandoned stack if a dependency callback calls $finish. */
    llg_string_move(target, line);
    return length > (size_t)INT_MAX ? INT_MAX : (int)length;
}

int llg_file_gets_packed(uint32_t descriptor, llg_ref_t* target) {
    if (!target || target->width == 0) return 0;
    llg_string_t value = {0};
    int result = llg_file_gets(descriptor, &value);
    if (result) llg_ref_write_owned(target, llg_string_to_packed(value, target->width,
                                                            target->is_signed));
    else llg_string_destroy(&value);
    return result;
}
