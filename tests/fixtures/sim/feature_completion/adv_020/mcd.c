/* ADV-020: vpi_mcd_open/close/name/printf/vprintf/flush over the $fopen
 * table (IEEE 1800-2009 38.24-38.29). Companion of mcd.sv. Every check
 * prints the result and the vpi_chk_error level the call left. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 level;

/* The error level of the routine just called, read before any other VPI
 * call clears it (38.2). */
static void keep_level(void) { level = vpi_chk_error(NULL); }

static int single_channel(PLI_UINT32 mcd) {
    return mcd != 0 && (mcd & (mcd - 1u)) == 0 && !(mcd & 1u) && !(mcd >> 31);
}

static PLI_UINT32 open_file(const char* name) {
    PLI_UINT32 mcd = vpi_mcd_open((PLI_BYTE8*)name);
    keep_level();
    return mcd;
}

static PLI_UINT32 close_mcd(PLI_UINT32 mcd) {
    PLI_UINT32 unclosed = vpi_mcd_close(mcd);
    keep_level();
    return unclosed;
}

static const char* name_of(PLI_UINT32 cd) {
    PLI_BYTE8* name = vpi_mcd_name(cd);
    keep_level();
    return name ? (const char*)name : "NULL";
}

static PLI_INT32 print_to(PLI_UINT32 mcd, const char* text) {
    PLI_INT32 written = vpi_mcd_printf(mcd, (PLI_BYTE8*)"%s", text);
    keep_level();
    return written;
}

static PLI_INT32 vprint_to(PLI_UINT32 mcd, const char* format, ...) {
    va_list ap;
    va_start(ap, format);
    PLI_INT32 written = vpi_mcd_vprintf(mcd, (PLI_BYTE8*)format, ap);
    va_end(ap);
    keep_level();
    return written;
}

static PLI_INT32 flush_mcd(PLI_UINT32 mcd) {
    PLI_INT32 result = vpi_mcd_flush(mcd);
    keep_level();
    return result;
}

static PLI_UINT32 int_argument(vpiHandle arguments) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(arguments), &value);
    return (PLI_UINT32)value.value.integer;
}

static PLI_UINT32 opened;

static PLI_INT32 mcd(PLI_BYTE8* data) {
    (void)data;
    vpiHandle arguments = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    PLI_UINT32 m = int_argument(arguments);
    PLI_UINT32 fd = int_argument(arguments);
    vpi_release_handle(arguments);
    PLI_UINT32 unused = UINT32_C(1) << 29;

    PLI_UINT32 a = open_file("adv020_v1.txt");
    vpi_printf("open v1: single=%d level=%d differs from m=%d\n", single_channel(a),
               (int)level, a != m);
    {
        int result = open_file("adv020_v1.txt") == a;
        vpi_printf("open v1 again: same=%d level=%d\n", result, (int)level);
    }
    {
        int result = open_file("adv020_hdl.txt") == m;
        vpi_printf("open the $fopen channel's file: same as m=%d level=%d\n", result, (int)level);
    }
    {
        PLI_UINT32 result = open_file("adv020_fd.txt");
        vpi_printf("open the $fopen fd's file: %u level=%d\n", (unsigned)result, (int)level);
    }
    {
        unsigned result = (unsigned)open_file(NULL);
        vpi_printf("open NULL: %u level=%d\n", result, (int)level);
    }
    {
        unsigned result = (unsigned)open_file("");
        vpi_printf("open empty: %u level=%d\n", result, (int)level);
    }
    {
        unsigned result = (unsigned)open_file("adv020_missing_dir/x.txt");
        vpi_printf("open in a missing directory: %u level=%d\n", result, (int)level);
    }

    {
        const char* result = name_of(a);
        vpi_printf("name a: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(m);
        vpi_printf("name m: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(fd);
        vpi_printf("name fd: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(1);
        vpi_printf("name 1: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(0x80000002u);
        vpi_printf("name stderr fd: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(0);
        vpi_printf("name 0: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(a | m);
        vpi_printf("name a|m: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(unused);
        vpi_printf("name unopened: %s level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(0x8000001fu);
        vpi_printf("name unopened fd: %s level=%d\n", result, (int)level);
    }
    PLI_BYTE8* first = vpi_mcd_name(a);
    PLI_BYTE8* second = vpi_mcd_name(m);
    vpi_printf("name buffer reused: %d now %s\n", first == second, (const char*)first);

    {
        int result = (int)print_to(a, "to a\n");
        vpi_printf("printf a: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)print_to(a | m, "to a and m\n");
        vpi_printf("printf a|m: %d level=%d\n", result, (int)level);
    }
    PLI_INT32 written = print_to(1, "to channel 1\n");
    vpi_printf("printf 1: %d level=%d\n", (int)written, (int)level);
    written = print_to(1 | a, "to channel 1 and a\n");
    vpi_printf("printf 1|a: %d level=%d\n", (int)written, (int)level);
    {
        int result = (int)print_to(0, "lost\n");
        vpi_printf("printf 0: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)print_to(unused, "lost\n");
        vpi_printf("printf unopened: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)print_to(a | unused, "partial\n");
        vpi_printf("printf a|unopened: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)print_to(fd, "lost\n");
        vpi_printf("printf fd: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)print_to(a, "");
        vpi_printf("printf empty: %d level=%d\n", result, (int)level);
    }
    char long_line[701];
    memset(long_line, 'x', 699);
    long_line[699] = '\n';
    long_line[700] = 0;
    {
        int result = (int)print_to(a, long_line);
        vpi_printf("printf 700 characters: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)vprint_to(a, "v=%s %d\n", "abc", 7);
        vpi_printf("vprintf a: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)vprint_to(unused, "lost %d\n", 1);
        vpi_printf("vprintf unopened: %d level=%d\n", result, (int)level);
    }
    written = vpi_mcd_printf(a, NULL);
    keep_level();
    vpi_printf("printf NULL format: %d level=%d\n", (int)written, (int)level);

    {
        int result = (int)flush_mcd(a);
        vpi_printf("flush a: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)flush_mcd(1 | a);
        vpi_printf("flush 1|a: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)flush_mcd(0);
        vpi_printf("flush 0: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)flush_mcd(unused);
        vpi_printf("flush unopened: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)flush_mcd(a | unused);
        vpi_printf("flush a|unopened: %d level=%d\n", result, (int)level);
    }
    {
        int result = (int)flush_mcd(fd);
        vpi_printf("flush fd: %d level=%d\n", result, (int)level);
    }

    {
        unsigned result = (unsigned)close_mcd(1);
        vpi_printf("close 1: %u level=%d\n", result, (int)level);
    }
    written = print_to(1, "channel 1 is still open\n");
    vpi_printf("printf 1 after close: %d level=%d\n", (int)written, (int)level);
    {
        unsigned result = (unsigned)close_mcd(0);
        vpi_printf("close 0: %u level=%d\n", result, (int)level);
    }
    {
        int result = close_mcd(unused) == unused;
        vpi_printf("close unopened: same=%d level=%d\n", result, (int)level);
    }
    {
        int result = close_mcd(fd) == fd;
        vpi_printf("close fd: same=%d level=%d\n", result, (int)level);
    }
    PLI_UINT32 b = open_file("adv020_v2.txt");
    print_to(b, "closed with an unopened channel\n");
    {
        int result = close_mcd(b | unused) == unused;
        vpi_printf("close b|unopened: unopened left=%d level=%d\n", result, (int)level);
    }
    {
        const char* result = name_of(b);
        vpi_printf("name b after close: %s level=%d\n", result, (int)level);
    }
    {
        int result = (int)print_to(b, "lost\n");
        vpi_printf("printf b after close: %d level=%d\n", result, (int)level);
    }
    {
        int result = close_mcd(b) == b;
        vpi_printf("close b again: same=%d level=%d\n", result, (int)level);
    }
    {
        int result = close_mcd(1 | b) == (1u | b);
        vpi_printf("close 1|b: 1|b left=%d level=%d\n", result, (int)level);
    }
    PLI_UINT32 reopened = open_file("adv020_v3.txt");
    vpi_printf("open v3 reuses b's channel: %d level=%d\n", reopened == b, (int)level);
    {
        unsigned result = (unsigned)close_mcd(reopened);
        vpi_printf("close v3: %u level=%d\n", result, (int)level);
    }
    opened = a;
    return 0;
}

static PLI_INT32 after_close(PLI_BYTE8* data) {
    (void)data;
    vpiHandle arguments = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    PLI_UINT32 m = int_argument(arguments);
    vpi_release_handle(arguments);
    {
        const char* result = name_of(m);
        vpi_printf("name m after $fclose: %s level=%d\n", result, (int)level);
    }
    {
        int result = close_mcd(m) == m;
        vpi_printf("close m after $fclose: same=%d level=%d\n", result, (int)level);
    }
    {
        unsigned result = (unsigned)close_mcd(opened);
        vpi_printf("close a: %u level=%d\n", result, (int)level);
    }
    return 0;
}

static PLI_INT32 vopen(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = (PLI_INT32)vpi_mcd_open((PLI_BYTE8*)"adv020_v4.txt");
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &value, NULL, vpiNoDelay);
    return 0;
}

/* Print a file the run wrote, line by line; the 700-character line is
 * summarized. */
static void show(const char* path) {
    FILE* file = fopen(path, "r");
    if (!file) {
        vpi_printf("[%s] missing\n", path);
        return;
    }
    vpi_printf("[%s]\n", path);
    char line[1024];
    while (fgets(line, sizeof(line), file)) {
        size_t length = strlen(line);
        if (length > 100)
            vpi_printf("| <%u characters>\n", (unsigned)length);
        else
            vpi_printf("| %s", line);
    }
    fclose(file);
}

static PLI_INT32 show_files(PLI_BYTE8* data) {
    (void)data;
    show("adv020_v1.txt");
    show("adv020_hdl.txt");
    show("adv020_fd.txt");
    show("adv020_v2.txt");
    show("adv020_v3.txt");
    show("adv020_v4.txt");
    return 0;
}

static void register_task(const char* name, PLI_INT32 (*calltf)(PLI_BYTE8*), PLI_INT32 type) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = type;
    data.sysfunctype = vpiIntFunc;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = calltf;
    vpi_register_systf(&data);
}

static void startup(void) {
    register_task("$mcd", mcd, vpiSysTask);
    register_task("$after_close", after_close, vpiSysTask);
    register_task("$vopen", vopen, vpiSysFunc);
    register_task("$show_files", show_files, vpiSysTask);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
