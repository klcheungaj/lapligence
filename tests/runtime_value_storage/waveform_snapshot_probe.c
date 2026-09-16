// Include the production writer to test private ownership transfers without
// thread timing assumptions, then exercise its public threaded VCD/FST API.
#include "llg_wave.c"

size_t storage_test_allocated(void);
size_t storage_test_released(void);

#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "snapshot check failed at line %d: %s\n", __LINE__, #condition); \
        exit(2); \
    } \
} while (0)

static void set_value(sv4_t* value, uint32_t width, uint64_t bits) {
    sv4_replace(value, sv4_zero(width, 0));
    if (width) value->bits[0] = bits;
}

static void check_released(void) {
    // Called only after close joins the worker, or during single-thread tests.
    CHECK(storage_test_allocated() == storage_test_released());
}

static char* read_file(const char* path) {
    FILE* file = fopen(path, "rb");
    CHECK(file != NULL);
    CHECK(fseek(file, 0, SEEK_END) == 0);
    long length = ftell(file);
    CHECK(length >= 0);
    CHECK(fseek(file, 0, SEEK_SET) == 0);
    char* text = (char*)malloc((size_t)length + 1u);
    CHECK(text != NULL);
    CHECK(fread(text, 1u, (size_t)length, file) == (size_t)length);
    text[length] = 0;
    CHECK(fclose(file) == 0);
    return text;
}

static void check_queue_transfers(void) {
    CHECK(sizeof(wave_event_t) < 128u);
    CHECK(llg_wave_model_init(1) == 0);
    sv4_t source = SV4_EMPTY;
    for (uint32_t round = 0; round < 3; round++) {
        // Fill the entire ring before consuming: source mutation after capture
        // deterministically precedes reading every snapshot, independent of OS
        // scheduling. Subsequent rounds reuse every slot.
        for (uint32_t i = 0; i < LLG_WAVE_QUEUE_CAP; i++) {
            set_value(&source, 65, (uint64_t)round * LLG_WAVE_QUEUE_CAP + i);
            source.bits[1] = 1;
            source.x[0] = 2;
            source.z[0] = 4;
            wave_event_t event = {0};
            event.kind = EV_CHANGE_SV4;
            CHECK(capture_sv4(&event, &source));
            CHECK(event.payload.sv4.bits != source.bits);
            uint64_t* allocation = event.payload.sv4.bits;
            queue_push(&event);
            CHECK(event.kind == EV_FILE && event.payload.sv4.bits == NULL);
            uint64_t head = atomic_u64_load(&g_wave.head);
            CHECK(g_wave.queue[(head - 1u) % LLG_WAVE_QUEUE_CAP].payload.sv4.bits
                  == allocation);
            set_value(&source, 1, 0);
        }
        CHECK(atomic_u64_load(&g_wave.head) - atomic_u64_load(&g_wave.tail)
              == LLG_WAVE_QUEUE_CAP);
        for (uint32_t i = 0; i < LLG_WAVE_QUEUE_CAP; i++) {
            uint64_t tail = atomic_u64_load(&g_wave.tail);
            wave_event_t event = queue_pop();
            CHECK(g_wave.queue[tail % LLG_WAVE_QUEUE_CAP].kind == EV_FILE);
            CHECK(g_wave.queue[tail % LLG_WAVE_QUEUE_CAP].payload.sv4.bits == NULL);
            CHECK(event.kind == EV_CHANGE_SV4 && event.payload.sv4.width == 65);
            CHECK(event.payload.sv4.bits[0] == (uint64_t)round * LLG_WAVE_QUEUE_CAP + i);
            CHECK(event.payload.sv4.bits[1] == 1);
            CHECK(event.payload.sv4.x[0] == 2 && event.payload.sv4.z[0] == 4);
            event_destroy(&event);
            CHECK(event.payload.sv4.bits == NULL);
        }
        sv4_destroy(&source);
        check_released();
    }
    CHECK(llg_wave_close(0) == 0);
    check_released();
}

static void check_vcd_snapshots(void) {
    const char* path = "snapshot-values.vcd";
    sv4_t packed = SV4_EMPTY, wide = SV4_EMPTY;
    sv4_t padded = SV4_EMPTY, empty = SV4_EMPTY;
    set_value(&packed, 8, 0x3c);
    set_value(&wide, 65, 0);
    wide.x[0] = 1;
    wide.z[1] = 1;
    set_value(&padded, 8, 0x5a);
    set_value(&empty, 0, 0);
    CHECK(llg_wave_model_init(1) == 0);
    CHECK(llg_wave_register_sv4("top\037packed", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037alias", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037wide", &wide, 65) == 0);
    CHECK(llg_wave_register_sv4("top\037padded", &padded, 65) == 0);
    CHECK(llg_wave_register_sv4("top\037empty", &empty, 1) == 0);
    llg_wave_file(path, 0);
    llg_wave_dumpvars(0);
    packed.bits[0] = 0xa5;
    llg_wave_changed_sv4(&packed, &packed, 1);
    packed.bits[0] = 0x5a;
    for (uint64_t now = 2; now <= 4096; now++) {
        packed.bits[0] = now & 0xffu;
        llg_wave_changed_sv4(&packed, &packed, now);
        packed.bits[0] = 0xff;
    }
    llg_wave_flush(4096);
    char* text = read_file(path);
    CHECK(strstr(text, " alias $end") != NULL);
    CHECK(strstr(text, "b00111100 ") != NULL);
    CHECK(strstr(text, "#1\nb10100101 ") != NULL);
    CHECK(strstr(text, "#4096\nb00000000 ") != NULL);
    char expected[70];
    expected[0] = 'b';
    memset(expected + 1, '0', 65);
    expected[1] = 'z';
    expected[65] = 'x';
    expected[66] = ' ';
    expected[67] = 0;
    CHECK(strstr(text, expected) != NULL);
    memset(expected + 1, '0', 65);
    memcpy(expected + 58, "01011010", 8);
    CHECK(strstr(text, expected) != NULL);
    free(text);
    CHECK(llg_wave_close(4096) == 0);
    CHECK(llg_wave_close(4096) == 0);
    sv4_destroy(&packed);
    sv4_destroy(&wide);
    sv4_destroy(&padded);
    sv4_destroy(&empty);
    check_released();
    CHECK(remove(path) == 0);
}

static void check_fst_snapshots(void) {
    const char* path = "snapshot-values.fst";
    sv4_t packed = SV4_EMPTY;
    set_value(&packed, 8, 0x3c);
    CHECK(llg_wave_model_init(1) == 0);
    CHECK(llg_wave_register_sv4("top\037packed", &packed, 8) == 0);
    CHECK(llg_wave_register_sv4("top\037alias", &packed, 8) == 0);
    llg_wave_file(path, 0);
    llg_wave_dumpvars(0);
    packed.bits[0] = 0xa5;
    llg_wave_changed_sv4(&packed, &packed, 7);
    packed.bits[0] = 0x5a;
    llg_wave_off(8);
    llg_wave_on(9);
    CHECK(llg_wave_close(9) == 0);
    sv4_destroy(&packed);
    check_released();
    void* reader = fstReaderOpen(path);
    CHECK(reader != NULL);
    CHECK(fstReaderGetMaxHandle(reader) == 1);
    CHECK(fstReaderGetVarCount(reader) == 2);
    char value[32];
    CHECK(fstReaderGetValueFromHandleAtTime(reader, 0, 1, value) != NULL);
    CHECK(strcmp(value, "00111100") == 0);
    CHECK(fstReaderGetValueFromHandleAtTime(reader, 7, 1, value) != NULL);
    CHECK(strcmp(value, "10100101") == 0);
    CHECK(fstReaderGetValueFromHandleAtTime(reader, 9, 1, value) != NULL);
    CHECK(strcmp(value, "01011010") == 0);
    fstReaderClose(reader);
    CHECK(remove(path) == 0);
}

static void check_discard_and_close(void) {
    sv4_t source = SV4_EMPTY;
    for (int cycle = 0; cycle < 6; cycle++) {
        int error_case = cycle % 3 == 0;
        const char* path = error_case ? "snapshot-error.unsupported" : "snapshot-close.vcd";
        set_value(&source, 8, 0);
        CHECK(llg_wave_model_init(1) == 0);
        CHECK(llg_wave_register_sv4("top\037packed", &source, 8) == 0);
        llg_wave_file(path, 0);
        if (cycle % 3 == 1) llg_wave_limit(1, 0);
        llg_wave_dumpvars(0);
        for (uint64_t now = 1; now <= 3000; now++) {
            source.bits[0] = now & 0xffu;
            llg_wave_changed_sv4(&source, &source, now);
        }
        if (error_case) llg_wave_flush(3000);
        // Non-error cases deliberately close without a flush: queued owners
        // must drain before registration and writer storage are destroyed.
        CHECK(llg_wave_close(3000) == (error_case ? -1 : 0));
        sv4_destroy(&source);
        check_released();
        if (!error_case) CHECK(remove(path) == 0);
    }
    CHECK(llg_wave_model_init(1) == 0);
    CHECK(llg_wave_close(0) == 0);
    check_released();
}

int main(void) {
    check_queue_transfers();
    check_vcd_snapshots();
    check_fst_snapshots();
    check_discard_and_close();
    printf("dynamic waveform snapshots: OK (event=%zu bytes)\n", sizeof(wave_event_t));
    return 0;
}
