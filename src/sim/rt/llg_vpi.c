// llg_vpi.c — bounded VPI object/registration bridge for generated models.

#include "llg_vpi.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

#define LLG_VPI_MAX_OBJECTS 16384
#define LLG_VPI_MAX_REGISTRATIONS 256
#define LLG_VPI_MAX_CALLBACKS 256
#define LLG_VPI_MAX_STARTUP_ROUTINES 1024
#define LLG_VPI_MAX_ARGS 256
#define LLG_VPI_MAX_HANDLES 65536
#define LLG_VPI_MAGIC 0x4c4c4756u

typedef enum {
    LLG_VPI_OBJECT = 1,
    LLG_VPI_ITERATOR = 2,
    LLG_VPI_SYSTF = 3,
    LLG_VPI_CALLBACK = 4,
    LLG_VPI_CALL = 5,
    LLG_VPI_ARGUMENT = 6,
} llg_vpi_handle_kind_t;

typedef struct llg_vpi_registration {
    s_vpi_systf_data data;
    char* name;
    vpiHandle public_handle;
} llg_vpi_registration_t;

typedef struct llg_vpi_callsite {
    uint64_t id;
    uint64_t time_unit_fs;
    llg_vpi_registration_t* registration;
    uint32_t result_width;
    int8_t result_signed;
    int8_t result_real;
    int valid;
    int arg_count;
    llg_vpi_compile_arg_t* args;
    struct llg_vpi_callsite* next;
} llg_vpi_callsite_t;

typedef struct llg_vpi_call {
    const char* name;
    llg_vpi_arg_t* args;
    int arg_count;
    llg_vpi_registration_t* registration;
    sv4_t return_value;
    double real_return;
    int has_real_return;
    int is_function;
    uint64_t time_unit_fs;
} llg_vpi_call_t;

typedef struct llg_vpi_handle {
    uint32_t magic;
    uint32_t generation;
    uint8_t kind;
    uint8_t dynamic;
    uint16_t reserved;
    llg_vpi_model_object_t* object;
    llg_vpi_registration_t* registration;
    llg_vpi_call_t* call;
    struct llg_vpi_handle* iterator;
    struct llg_vpi_handle* next;
    struct llg_vpi_handle* all_next;
    llg_vpi_call_t* owner_call;
    vpiHandle* items;
    int item_count;
    int item_index;
    int iterator_type;
    void* userdata;
    s_cb_data callback;
    llg_vpi_arg_t* argument;
    int argument_index;
} llg_vpi_handle_t;

typedef struct {
    const char* design_name;
    uint32_t generation;
    llg_vpi_model_object_t* objects;
    size_t object_count;
    llg_vpi_handle_t* object_handles;
    llg_vpi_registration_t registrations[LLG_VPI_MAX_REGISTRATIONS];
    int registration_count;
    llg_vpi_handle_t* registration_handles;
    llg_vpi_handle_t* callbacks;
    int callback_count;
    size_t handle_count;
    llg_vpi_handle_t* all_handles;
    int startup_loaded;
    int startup_called;
    int running;
    int failed;
    int error_pending;
    int error_state;
    int error_level;
    int error_line;
    char error_message[512];
    char error_code[64];
    char error_file[256];
    char error_product[32];
#if defined(_WIN32)
    HMODULE plugin_handles[16];
#else
    void* plugin_handles[16];
#endif
    int plugin_count;
    llg_vpi_call_t* active_call;
    llg_vpi_callsite_t* callsites;
    size_t callsite_count;
    s_vpi_vecval* value_vector;   /* Simulator-owned vpi_get_value result. */
    size_t value_vector_capacity;
} llg_vpi_state_t;

static llg_vpi_state_t g_vpi;
static uint32_t g_vpi_generation;

static void vpi_set_error(int state, int level, const char* code,
                          const char* message) {
    g_vpi.error_pending = 1;
    g_vpi.error_state = state;
    g_vpi.error_level = level;
    g_vpi.error_line = 0;
    snprintf(g_vpi.error_code, sizeof(g_vpi.error_code), "%s",
             code ? code : "LLG_VPI");
    snprintf(g_vpi.error_message, sizeof(g_vpi.error_message), "%s",
             message ? message : "VPI operation failed");
}

static void vpi_set_errorf(int state, int level, const char* code,
                           const char* format, ...) {
    va_list ap;
    g_vpi.error_pending = 1;
    g_vpi.error_state = state;
    g_vpi.error_level = level;
    g_vpi.error_line = 0;
    snprintf(g_vpi.error_code, sizeof(g_vpi.error_code), "%s",
             code ? code : "LLG_VPI");
    va_start(ap, format);
    vsnprintf(g_vpi.error_message, sizeof(g_vpi.error_message), format, ap);
    va_end(ap);
}

static char* vpi_strdup(const char* text) {
    if (!text) return NULL;
    size_t length = strlen(text);
    if (length == SIZE_MAX) return NULL;
    char* copy = (char*)malloc(length + 1);
    if (!copy) return NULL;
    memcpy(copy, text, length + 1);
    return copy;
}

static void vpi_fail_runtime(const char* message) {
    g_vpi.failed = 1;
    vpi_set_error(vpiRun, vpiError, "LLG_VPI_RUNTIME", message);
    llg_rt_request_finish();
}

/* A plugin can pass an arbitrary C pointer as a vpiHandle.  Do not
 * dereference it until it has been matched against storage owned by this
 * runtime: checking the magic word first would turn a malformed handle into
 * an access violation rather than a checked VPI error. */
static llg_vpi_handle_t* known_handle(vpiHandle handle) {
    if (!handle) return NULL;
    uintptr_t address = (uintptr_t)handle;
    if (g_vpi.object_handles &&
        g_vpi.object_count <= SIZE_MAX / sizeof(*g_vpi.object_handles)) {
        uintptr_t begin = (uintptr_t)g_vpi.object_handles;
        uintptr_t bytes = g_vpi.object_count * sizeof(*g_vpi.object_handles);
        uintptr_t end = begin + bytes;
        if (end >= begin && address >= begin && address < end &&
            (address - begin) % sizeof(*g_vpi.object_handles) == 0)
            return (llg_vpi_handle_t*)handle;
    }
    for (llg_vpi_handle_t* cursor = g_vpi.all_handles; cursor;
         cursor = cursor->all_next) {
        if (cursor == (llg_vpi_handle_t*)handle) return cursor;
    }
    return NULL;
}

static int valid_handle(vpiHandle handle, llg_vpi_handle_kind_t kind) {
    llg_vpi_handle_t* object = known_handle(handle);
    if (!object || object->magic != LLG_VPI_MAGIC ||
        object->generation != g_vpi.generation ||
        object->kind != (uint8_t)kind)
        return 0;
    return 1;
}

static llg_vpi_handle_t* make_handle(llg_vpi_handle_kind_t kind, int dynamic) {
    if (g_vpi.handle_count >= LLG_VPI_MAX_HANDLES) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_LIMIT", "VPI handle limit exceeded");
        g_vpi.failed = 1;
        return NULL;
    }
    llg_vpi_handle_t* handle = (llg_vpi_handle_t*)calloc(1, sizeof(*handle));
    if (!handle) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_NOMEM", "VPI handle allocation failed");
        g_vpi.failed = 1;
        return NULL;
    }
    handle->magic = LLG_VPI_MAGIC;
    handle->generation = g_vpi.generation;
    handle->kind = (uint8_t)kind;
    handle->dynamic = (uint8_t)dynamic;
    handle->all_next = g_vpi.all_handles;
    g_vpi.all_handles = handle;
    ++g_vpi.handle_count;
    return handle;
}

static void invalidate_handle(llg_vpi_handle_t* handle) {
    if (!handle) return;
    handle->magic = 0;
    handle->generation = 0;
}

static llg_vpi_handle_t* object_handle(llg_vpi_model_object_t* object) {
    if (!object || object < g_vpi.objects ||
        object >= g_vpi.objects + g_vpi.object_count)
        return NULL;
    return &g_vpi.object_handles[object - g_vpi.objects];
}

static llg_vpi_model_object_t* object_from_handle(vpiHandle handle) {
    if (!valid_handle(handle, LLG_VPI_OBJECT)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid or stale VPI object handle");
        return NULL;
    }
    return ((llg_vpi_handle_t*)handle)->object;
}

static llg_vpi_registration_t* registration_from_handle(vpiHandle handle) {
    if (!valid_handle(handle, LLG_VPI_SYSTF)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid system-function handle");
        return NULL;
    }
    return ((llg_vpi_handle_t*)handle)->registration;
}

static llg_vpi_handle_t* call_handle(void) {
    if (!g_vpi.active_call) return NULL;
    llg_vpi_handle_t* handle = make_handle(LLG_VPI_CALL, 1);
    if (handle) {
        handle->call = g_vpi.active_call;
        handle->owner_call = g_vpi.active_call;
    }
    return handle;
}

static llg_vpi_arg_t* argument_from_handle(vpiHandle handle) {
    if (!valid_handle(handle, LLG_VPI_ARGUMENT)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI argument handle");
        return NULL;
    }
    return ((llg_vpi_handle_t*)handle)->argument;
}

static int type_matches(const llg_vpi_model_object_t* object, int type) {
    if (!object) return 0;
    if (type == vpiVariables) return object->type == vpiReg || object->type == vpiRealVar;
    if (type == vpiInternalScope) return object->type == vpiModule;
    return object->type == type;
}

static int object_is_descendant(const llg_vpi_model_object_t* object,
                                const llg_vpi_model_object_t* parent) {
    if (!parent) return object->parent == NULL;
    return object && object->parent == parent;
}

static int callback_reason_supported(int reason) {
    return reason == cbStartOfSimulation || reason == cbEndOfSimulation;
}

static llg_vpi_registration_t* find_registration(const char* name) {
    if (!name) return NULL;
    for (int i = 0; i < g_vpi.registration_count; ++i) {
        if (strcmp(g_vpi.registrations[i].name, name) == 0)
            return &g_vpi.registrations[i];
    }
    return NULL;
}

static int registration_type_is_function(const llg_vpi_registration_t* registration) {
    return registration && registration->data.type == vpiSysFunc;
}

static void destroy_dynamic_handle(llg_vpi_handle_t* handle) {
    if (!handle || !handle->dynamic) return;
    if (handle->items) {
        if (handle->kind == LLG_VPI_ITERATOR) {
            for (int i = 0; i < handle->item_count; ++i) {
                llg_vpi_handle_t* item = (llg_vpi_handle_t*)handle->items[i];
                if (item && item->kind == LLG_VPI_ARGUMENT &&
                    item->magic == LLG_VPI_MAGIC) {
                    invalidate_handle(item);
                    item->dynamic = 0;
                }
            }
        }
        free(handle->items);
        handle->items = NULL;
    }
    invalidate_handle(handle);
    handle->dynamic = 0;
}

/* Call and argument handles borrow the generated call's argument array.  The
 * call record is stack-owned, so all handles associated with it become
 * tombstones before that record goes out of scope. */
static void invalidate_call_handles(llg_vpi_call_t* call) {
    if (!call) return;
    for (llg_vpi_handle_t* cursor = g_vpi.all_handles; cursor;
         cursor = cursor->all_next) {
        if (cursor->call != call && cursor->owner_call != call) continue;
        if (cursor->items) {
            if (cursor->kind == LLG_VPI_ITERATOR) {
                for (int i = 0; i < cursor->item_count; ++i) {
                    llg_vpi_handle_t* item = (llg_vpi_handle_t*)cursor->items[i];
                    if (item && item->owner_call == call) {
                        invalidate_handle(item);
                        item->dynamic = 0;
                    }
                }
            }
            free(cursor->items);
            cursor->items = NULL;
        }
        invalidate_handle(cursor);
        cursor->dynamic = 0;
    }
}

static int model_pointer_in_range(const llg_vpi_model_object_t* pointer,
                                  const llg_vpi_model_object_t* objects,
                                  size_t object_count) {
    if (!pointer || !objects || object_count > SIZE_MAX / sizeof(*objects)) return 0;
    uintptr_t begin = (uintptr_t)objects;
    uintptr_t bytes = object_count * sizeof(*objects);
    uintptr_t address = (uintptr_t)pointer;
    uintptr_t end = begin + bytes;
    return end >= begin && address >= begin && address < end &&
           (address - begin) % sizeof(*objects) == 0;
}

static int model_object_valid(const llg_vpi_model_object_t* object,
                              const llg_vpi_model_object_t* objects,
                              size_t object_count) {
    if (!object || !object->name || !object->full_name ||
        !model_pointer_in_range(object, objects, object_count) ||
        (object->parent &&
         (!model_pointer_in_range(object->parent, objects, object_count) ||
          object->parent->type != vpiModule)) ||
        object->width > LLG_MAX_WIDTH)
        return 0;
    switch (object->type) {
        case vpiModule:
            return object->width == 0 && !object->is_real && !object->is_net &&
                   !object->packed && !object->real;
        case vpiNet:
            return object->width != 0 && !object->is_real && object->is_net &&
                   object->packed && !object->real;
        case vpiReg:
            return object->width != 0 && !object->is_real && !object->is_net &&
                   object->packed && !object->real;
        case vpiRealVar:
            return object->width == 0 && object->is_real && !object->is_net &&
                   !object->packed && object->real;
        case vpiRegArray:
            return (object->is_real ? object->width == 0 : object->width != 0) &&
                   !object->is_net;
        default:
            return 0;
    }
}

int llg_vpi_model_init(const char* design_name,
                       llg_vpi_model_object_t* objects, size_t object_count) {
    if (g_vpi.objects || !objects || object_count == 0 ||
        object_count > LLG_VPI_MAX_OBJECTS) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_MODEL", "invalid or duplicate VPI model metadata");
        return 0;
    }
    for (size_t i = 0; i < object_count; ++i) {
        if (!model_object_valid(&objects[i], objects, object_count)) {
            vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_MODEL",
                           "invalid VPI model object at index %zu", i);
            return 0;
        }
    }
    memset(&g_vpi, 0, sizeof(g_vpi));
    ++g_vpi_generation;
    if (g_vpi_generation == 0) ++g_vpi_generation;
    g_vpi.generation = g_vpi_generation;
    g_vpi.design_name = design_name ? design_name : "llg";
    g_vpi.objects = objects;
    g_vpi.object_count = object_count;
    g_vpi.object_handles = (llg_vpi_handle_t*)calloc(object_count, sizeof(*g_vpi.object_handles));
    if (!g_vpi.object_handles) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI object table allocation failed");
        return 0;
    }
    for (size_t i = 0; i < object_count; ++i) {
        g_vpi.object_handles[i].magic = LLG_VPI_MAGIC;
        g_vpi.object_handles[i].generation = g_vpi.generation;
        g_vpi.object_handles[i].kind = LLG_VPI_OBJECT;
        g_vpi.object_handles[i].object = &objects[i];
    }
    snprintf(g_vpi.error_product, sizeof(g_vpi.error_product), "lapligence");
    return 1;
}

static int load_one_plugin(const char* path) {
    if (!path || !*path || g_vpi.plugin_count >= 16) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "empty or excessive VPI plugin path");
        return 0;
    }
#if defined(_WIN32)
    HMODULE module = LoadLibraryA(path);
    if (!module) {
        vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "cannot load VPI plugin `%s`", path);
        return 0;
    }
    void (**startup)(void) = (void (**)(void))GetProcAddress(module, "vlog_startup_routines");
#else
    void* module = dlopen(path, RTLD_NOW | RTLD_GLOBAL);
    if (!module) {
        const char* loader_error = dlerror();
        vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "cannot load VPI plugin `%s`: %s", path,
                       loader_error ? loader_error : "unknown loader error");
        return 0;
    }
    void (**startup)(void) = (void (**)(void))dlsym(module, "vlog_startup_routines");
#endif
    if (!startup) {
        vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "VPI plugin `%s` exports no startup table", path);
#if defined(_WIN32)
        FreeLibrary(module);
#else
        dlclose(module);
#endif
        return 0;
    }
    g_vpi.plugin_handles[g_vpi.plugin_count++] = module;
    int terminated = 0;
    for (int i = 0; i < LLG_VPI_MAX_STARTUP_ROUTINES; ++i) {
        if (!startup[i]) {
            terminated = 1;
            break;
        }
        startup[i]();
    }
    if (!terminated) {
        --g_vpi.plugin_count;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "VPI startup table exceeds 1024 entries");
#if defined(_WIN32)
        FreeLibrary(module);
#else
        dlclose(module);
#endif
        return 0;
    }
    g_vpi.startup_loaded = 1;
    return 1;
}

int llg_vpi_startup(void) {
    if (!g_vpi.objects) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_MODEL", "VPI startup ran without model metadata");
        return 0;
    }
    if (g_vpi.startup_called) return !g_vpi.failed;
    g_vpi.startup_called = 1;
    const char* paths = getenv("LLG_VPI_PLUGIN");
    if (paths && *paths) {
        char buffer[4096];
        size_t length = strlen(paths);
        if (length >= sizeof(buffer)) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_PLUGIN", "LLG_VPI_PLUGIN exceeds 4095 bytes");
            return 0;
        }
        memcpy(buffer, paths, length + 1);
        char* cursor = buffer;
        while (cursor && *cursor) {
#if defined(_WIN32)
            char* next = strchr(cursor, ';');
#else
            char* next = strchr(cursor, ':');
#endif
            if (next) *next++ = '\0';
            if (!load_one_plugin(cursor)) return 0;
            cursor = next;
        }
    }
    return !g_vpi.failed && !g_vpi.error_pending;
}

vpiHandle vpi_register_systf(p_vpi_systf_data data) {
    if (!data || !data->tfname || data->tfname[0] != '$' ||
        (data->type != vpiSysTask && data->type != vpiSysFunc) ||
        !data->calltf || g_vpi.registration_count >= LLG_VPI_MAX_REGISTRATIONS) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_REGISTRATION", "invalid VPI system-task/function registration");
        return NULL;
    }
    if (find_registration((const char*)data->tfname)) {
        vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_DUPLICATE", "VPI system function `%s` registered twice", data->tfname);
        return NULL;
    }
    if (data->type == vpiSysFunc &&
        data->sysfunctype != vpiIntFunc && data->sysfunctype != vpiRealFunc &&
        data->sysfunctype != vpiSizedFunc &&
        data->sysfunctype != vpiSizedSignedFunc) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_REGISTRATION", "unsupported VPI system-function result type");
        return NULL;
    }
    llg_vpi_registration_t* registration = &g_vpi.registrations[g_vpi.registration_count++];
    memset(registration, 0, sizeof(*registration));
    registration->name = vpi_strdup((const char*)data->tfname);
    if (!registration->name) {
        --g_vpi.registration_count;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_NOMEM", "VPI registration name allocation failed");
        return NULL;
    }
    registration->data = *data;
    registration->data.tfname = (PLI_BYTE8*)registration->name;
    llg_vpi_handle_t* handle = make_handle(LLG_VPI_SYSTF, 0);
    if (!handle) {
        free(registration->name);
        registration->name = NULL;
        --g_vpi.registration_count;
        return NULL;
    }
    handle->registration = registration;
    registration->public_handle = handle;
    return handle;
}

void vpi_get_systf_info(vpiHandle object, p_vpi_systf_data data) {
    llg_vpi_registration_t* registration = registration_from_handle(object);
    if (!registration || !data) {
        if (!data) vpi_set_error(vpiPLI, vpiError, "LLG_VPI_ARGUMENT", "null vpi_get_systf_info output");
        return;
    }
    *data = registration->data;
}

static vpiHandle find_object(const char* name, const llg_vpi_model_object_t* scope) {
    if (!name || !*name) return NULL;
    char full[4096];
    if (scope && scope->full_name) {
        int written = snprintf(full, sizeof(full), "%s.%s", scope->full_name, name);
        if (written >= 0 && (size_t)written < sizeof(full)) {
            for (size_t i = 0; i < g_vpi.object_count; ++i) {
                if (strcmp(g_vpi.objects[i].full_name, full) == 0)
                    return object_handle(&g_vpi.objects[i]);
            }
        }
    }
    int written = snprintf(full, sizeof(full), "%s", name);
    if (written < 0 || (size_t)written >= sizeof(full)) return NULL;
    for (size_t i = 0; i < g_vpi.object_count; ++i) {
        if (strcmp(g_vpi.objects[i].full_name, full) == 0)
            return object_handle(&g_vpi.objects[i]);
    }
    if (!scope && strchr(name, '.') == NULL) {
        vpiHandle found = NULL;
        for (size_t i = 0; i < g_vpi.object_count; ++i) {
            if (strcmp(g_vpi.objects[i].name, name) == 0) {
                if (found) {
                    vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_AMBIGUOUS", "VPI name `%s` is ambiguous", name);
                    return NULL;
                }
                found = object_handle(&g_vpi.objects[i]);
            }
        }
        return found;
    }
    return NULL;
}

vpiHandle vpi_handle_by_name(PLI_BYTE8* name, vpiHandle scope) {
    const llg_vpi_model_object_t* parent = NULL;
    if (scope) {
        parent = object_from_handle(scope);
        if (!parent) return NULL;
        if (parent->type != vpiModule) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_SCOPE", "VPI name lookup scope is not a module");
            return NULL;
        }
    }
    vpiHandle result = find_object((const char*)name, parent);
    if (!result)
        vpi_set_errorf(vpiPLI, vpiError, "LLG_VPI_NOT_FOUND", "VPI object `%s` was not found", name ? (char*)name : "<null>");
    return result;
}

vpiHandle vpi_handle(PLI_INT32 type, vpiHandle ref_handle) {
    if (type == vpiSysTfCall || type == vpiSysFuncCall) {
        if (ref_handle) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_RELATION", "system-call handle relation has no reference object");
            return NULL;
        }
        return call_handle();
    }
    llg_vpi_model_object_t* object = object_from_handle(ref_handle);
    if (!object) return NULL;
    if (type == vpiParent || type == vpiScope) {
        return object_handle(object->parent);
    }
    vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI one-to-one relation");
    return NULL;
}

vpiHandle vpi_handle_by_index(vpiHandle object, PLI_INT32 index) {
    (void)object;
    (void)index;
    vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "indexed VPI lookup is not supported");
    return NULL;
}

vpiHandle vpi_iterate(PLI_INT32 type, vpiHandle ref_handle) {
    if (type == vpiArgument && valid_handle(ref_handle, LLG_VPI_CALL)) {
        llg_vpi_call_t* call = ((llg_vpi_handle_t*)ref_handle)->call;
        if (!call || call->arg_count <= 0) return NULL;
        llg_vpi_handle_t* iterator = make_handle(LLG_VPI_ITERATOR, 1);
        if (!iterator) return NULL;
        iterator->owner_call = call;
        iterator->items = (vpiHandle*)calloc((size_t)call->arg_count, sizeof(*iterator->items));
        if (!iterator->items) {
            destroy_dynamic_handle(iterator);
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_NOMEM", "VPI argument iterator allocation failed");
            return NULL;
        }
        iterator->item_count = call->arg_count;
        iterator->iterator_type = vpiArgument;
        for (int i = 0; i < call->arg_count; ++i) {
            llg_vpi_handle_t* argument = make_handle(LLG_VPI_ARGUMENT, 1);
            if (!argument) {
                for (int j = 0; j < i; ++j) destroy_dynamic_handle((llg_vpi_handle_t*)iterator->items[j]);
                destroy_dynamic_handle(iterator);
                return NULL;
            }
            argument->argument = &call->args[i];
            argument->owner_call = call;
            argument->argument_index = i;
            iterator->items[i] = argument;
        }
        return iterator;
    }
    llg_vpi_model_object_t* parent = NULL;
    if (ref_handle) {
        parent = object_from_handle(ref_handle);
        if (!parent) return NULL;
    }
    if (type != vpiModule && type != vpiNet && type != vpiReg &&
        type != vpiRealVar && type != vpiRegArray && type != vpiVariables &&
        type != vpiInternalScope) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI iteration type");
        return NULL;
    }
    vpiHandle* items = (vpiHandle*)calloc(g_vpi.object_count, sizeof(*items));
    if (!items) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_NOMEM", "VPI iterator allocation failed");
        return NULL;
    }
    int count = 0;
    for (size_t i = 0; i < g_vpi.object_count; ++i) {
        llg_vpi_model_object_t* candidate = &g_vpi.objects[i];
        if (type_matches(candidate, type) && object_is_descendant(candidate, parent))
            items[count++] = object_handle(candidate);
    }
    if (count == 0) {
        free(items);
        return NULL;
    }
    llg_vpi_handle_t* iterator = make_handle(LLG_VPI_ITERATOR, 1);
    if (!iterator) {
        free(items);
        return NULL;
    }
    iterator->items = items;
    iterator->item_count = count;
    iterator->iterator_type = type;
    return iterator;
}

vpiHandle vpi_scan(vpiHandle iterator_handle) {
    if (!valid_handle(iterator_handle, LLG_VPI_ITERATOR)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI iterator handle");
        return NULL;
    }
    llg_vpi_handle_t* iterator = (llg_vpi_handle_t*)iterator_handle;
    if (iterator->item_index >= iterator->item_count) {
        return NULL;
    }
    return iterator->items[iterator->item_index++];
}

PLI_INT32 vpi_get(PLI_INT32 property, vpiHandle handle) {
    if (valid_handle(handle, LLG_VPI_OBJECT)) {
        llg_vpi_model_object_t* object = ((llg_vpi_handle_t*)handle)->object;
        switch (property) {
            case vpiType: return object->type;
            case vpiSize: return (PLI_INT32)object->width;
            case vpiScalar: return object->width == 1;
            case vpiVector: return object->width > 1;
            case vpiSigned: return object->is_signed != 0;
            case vpiValid: return vpiValidTrue;
            case vpiTopModule: return object->type == vpiModule && object->parent == NULL;
            case vpiNetType: return object->is_net ? vpiWire : 0;
            case vpiLineNo: return object->line;
            default:
                vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI object property");
                return vpiUndefined;
        }
    }
    if (valid_handle(handle, LLG_VPI_SYSTF)) {
        llg_vpi_registration_t* registration = ((llg_vpi_handle_t*)handle)->registration;
        if (property == vpiType) return vpiUserSystf;
        if (property == vpiUserDefn) return vpiValidTrue;
        if (property == vpiSysFuncType) return registration->data.sysfunctype;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI system-function property");
        return vpiUndefined;
    }
    if (valid_handle(handle, LLG_VPI_ITERATOR)) {
        llg_vpi_handle_t* iterator = (llg_vpi_handle_t*)handle;
        if (property == vpiType) return vpiIterator;
        if (property == vpiIteratorType) return iterator->iterator_type;
        if (property == vpiValid) return vpiValidTrue;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI iterator property");
        return vpiUndefined;
    }
    if (valid_handle(handle, LLG_VPI_CALLBACK)) {
        if (property == vpiType) return vpiCallback;
        if (property == vpiValid) return vpiValidTrue;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI callback property");
        return vpiUndefined;
    }
    if (valid_handle(handle, LLG_VPI_CALL)) {
        llg_vpi_call_t* call = ((llg_vpi_handle_t*)handle)->call;
        if (property == vpiType) return call->is_function ? vpiSysFuncCall : vpiSysTaskCall;
        if (property == vpiSize) return call->has_real_return ? 0 : (PLI_INT32)call->return_value.width;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI system-call property");
        return vpiUndefined;
    }
    if (valid_handle(handle, LLG_VPI_ARGUMENT)) {
        llg_vpi_arg_t* argument = ((llg_vpi_handle_t*)handle)->argument;
        if (property == vpiType) return argument->is_real ? vpiRealVar : vpiReg;
        if (property == vpiSize) return argument->is_real ? 0 : (PLI_INT32)argument->width;
        if (property == vpiScalar) return !argument->is_real && argument->width == 1;
        if (property == vpiVector) return !argument->is_real && argument->width > 1;
        if (property == vpiSigned) return argument->is_signed != 0;
        if (property == vpiValid) return vpiValidTrue;
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI argument property");
        return vpiUndefined;
    }
    vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI handle in vpi_get");
    return vpiUndefined;
}

PLI_INT64 vpi_get64(PLI_INT32 property, vpiHandle handle) {
    return (PLI_INT64)vpi_get(property, handle);
}

PLI_BYTE8* vpi_get_str(PLI_INT32 property, vpiHandle handle) {
    if (valid_handle(handle, LLG_VPI_OBJECT)) {
        llg_vpi_model_object_t* object = ((llg_vpi_handle_t*)handle)->object;
        switch (property) {
            case vpiName: return (PLI_BYTE8*)object->name;
            case vpiFullName: return (PLI_BYTE8*)object->full_name;
            case vpiDefName: return (PLI_BYTE8*)object->definition_name;
            case vpiFile: return (PLI_BYTE8*)object->file;
            default: break;
        }
    } else if (valid_handle(handle, LLG_VPI_SYSTF) && property == vpiName) {
        return (PLI_BYTE8*)((llg_vpi_handle_t*)handle)->registration->name;
    } else if (valid_handle(handle, LLG_VPI_CALL) && property == vpiName) {
        return (PLI_BYTE8*)((llg_vpi_handle_t*)handle)->call->name;
    } else if (valid_handle(handle, LLG_VPI_ARGUMENT) && property == vpiName) {
        static char argument_name[32];
        snprintf(argument_name, sizeof(argument_name), "arg%d",
                 ((llg_vpi_handle_t*)handle)->argument_index);
        return (PLI_BYTE8*)argument_name;
    }
    llg_vpi_handle_t* known = known_handle(handle);
    if (!known || known->magic != LLG_VPI_MAGIC ||
        known->generation != g_vpi.generation) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE",
                      "invalid VPI handle in vpi_get_str");
    } else {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED",
                      "unsupported VPI string property");
    }
    return NULL;
}

static sv4_t call_value(llg_vpi_call_t* call) {
    if (!call) return sv4_x(1, 0);
    return call->return_value;
}

static void copy_to_vector(sv4_t value, s_vpi_vecval* vector) {
    if (!vector) return;
    uint32_t words = (value.width + 31u) / 32u;
    for (uint32_t word = 0; word < words; ++word) {
        uint32_t aval = 0;
        uint32_t bval = 0;
        for (uint32_t bit = 0; bit < 32; ++bit) {
            uint32_t index = word * 32u + bit;
            if (index >= value.width) break;
            uint32_t limb = index / 64u;
            uint32_t offset = index % 64u;
            uint32_t known = (uint32_t)((value.bits[limb] >> offset) & 1u);
            uint32_t unknown = (uint32_t)(((value.x[limb] | value.z[limb]) >> offset) & 1u);
            if (known) aval |= 1u << bit;
            if (unknown) bval |= 1u << bit;
            if (unknown && ((value.x[limb] >> offset) & 1u)) aval |= 1u << bit;
        }
        vector[word].aval = aval;
        vector[word].bval = bval;
    }
}

static sv4_t handle_value(vpiHandle handle, int* valid) {
    *valid = 0;
    if (valid_handle(handle, LLG_VPI_OBJECT)) {
        llg_vpi_model_object_t* object = ((llg_vpi_handle_t*)handle)->object;
        if (object->is_real || !object->packed) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED",
                          "VPI object has no packed scalar/vector storage");
            return sv4_x(1, 0);
        }
        *valid = 1;
        return *object->packed;
    }
    if (valid_handle(handle, LLG_VPI_CALL)) {
        llg_vpi_call_t* call = ((llg_vpi_handle_t*)handle)->call;
        if (call->has_real_return) return sv4_x(1, 0);
        *valid = 1;
        return call_value(call);
    }
    if (valid_handle(handle, LLG_VPI_ARGUMENT)) {
        llg_vpi_arg_t* argument = argument_from_handle(handle);
        if (!argument || argument->is_real) return sv4_x(1, 0);
        *valid = 1;
        return argument->packed;
    }
    vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "handle has no packed value");
    return sv4_x(1, 0);
}

void vpi_get_value(vpiHandle handle, p_vpi_value output) {
    if (!output) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_ARGUMENT", "null vpi_get_value output");
        return;
    }
    if (valid_handle(handle, LLG_VPI_CALL) &&
        !((llg_vpi_handle_t*)handle)->call->is_function) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED",
                      "system-task handles do not have values");
        return;
    }
    int valid = 0;
    double real = 0.0;
    if (valid_handle(handle, LLG_VPI_OBJECT) && ((llg_vpi_handle_t*)handle)->object->is_real) {
        llg_vpi_model_object_t* object = ((llg_vpi_handle_t*)handle)->object;
        if (!object->real) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_VALUE", "real handle has no storage");
            return;
        }
        if (output->format != vpiRealVal) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_VALUE", "real handle requires vpiRealVal");
            return;
        }
        real = *object->real;
        valid = 1;
    } else if (valid_handle(handle, LLG_VPI_ARGUMENT) && ((llg_vpi_handle_t*)handle)->argument->is_real) {
        if (output->format != vpiRealVal) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_VALUE", "real argument requires vpiRealVal");
            return;
        }
        real = ((llg_vpi_handle_t*)handle)->argument->real;
        valid = 1;
    } else if (valid_handle(handle, LLG_VPI_CALL) && ((llg_vpi_handle_t*)handle)->call->has_real_return) {
        if (output->format != vpiRealVal) {
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_VALUE", "real system-function requires vpiRealVal");
            return;
        }
        real = ((llg_vpi_handle_t*)handle)->call->real_return;
        valid = 1;
    }
    if (valid && output->format == vpiRealVal) {
        output->value.real = real;
        return;
    }
    sv4_t value = handle_value(handle, &valid);
    if (!valid) return;
    switch (output->format) {
        case vpiScalarVal:
            if (value.width != 1) {
                vpi_set_error(vpiPLI, vpiError, "LLG_VPI_VALUE", "scalar value requested for a vector handle");
                return;
            }
            output->value.scalar = value.z[0] ? vpiZ : value.x[0] ? vpiX : value.bits[0] ? vpi1 : vpi0;
            break;
        case vpiIntVal:
            output->value.integer = (PLI_INT32)sv4_to_i64(value);
            break;
        case vpiVectorVal: {
            /* The request supplies only a format, not a writable vector pointer.
             * Keep the result alive until the next get-value call or shutdown. */
            output->value.vector = NULL;
            if (value.width == 0 || value.width > LLG_MAX_WIDTH) {
                vpi_set_error(vpiPLI, vpiError, "LLG_VPI_VALUE", "invalid packed value width");
                return;
            }
            size_t words = ((size_t)value.width - 1u) / 32u + 1u;
            if (words > SIZE_MAX / sizeof(*g_vpi.value_vector)) {
                vpi_set_error(vpiPLI, vpiError, "LLG_VPI_NOMEM", "VPI vector size overflow");
                return;
            }
            if (words > g_vpi.value_vector_capacity) {
                s_vpi_vecval* vector = (s_vpi_vecval*)realloc(
                    g_vpi.value_vector, words * sizeof(*vector));
                if (!vector) {
                    vpi_set_error(vpiPLI, vpiError, "LLG_VPI_NOMEM", "VPI vector allocation failed");
                    return;
                }
                g_vpi.value_vector = vector;
                g_vpi.value_vector_capacity = words;
            }
            copy_to_vector(value, g_vpi.value_vector);
            output->value.vector = g_vpi.value_vector;
            break;
        }
        case vpiBinStrVal:
        case vpiOctStrVal:
        case vpiDecStrVal:
        case vpiHexStrVal: {
            static char text[LLG_VPI_MAX_OBJECTS];
            char format = output->format == vpiBinStrVal ? 'b' : output->format == vpiOctStrVal ? 'o' : output->format == vpiHexStrVal ? 'h' : 'd';
            sv4_format(format, value, text, sizeof(text));
            output->value.str = (PLI_BYTE8*)text;
            break;
        }
        case vpiStringVal:
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED",
                          "string VPI values are outside the bounded API");
            break;
        case vpiSuppressVal: break;
        default:
            vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported VPI value format");
            break;
    }
}

static int value_from_vpi(const s_vpi_value* input, uint32_t width,
                          int8_t is_signed, sv4_t* result) {
    if (!input || !result) return 0;
    switch (input->format) {
        case vpiScalarVal:
            if (width != 1) return 0;
            *result = input->value.scalar == vpiZ ? sv4_fill(3, width, is_signed) : input->value.scalar == vpiX ? sv4_fill(2, width, is_signed) : sv4_from_u64(input->value.scalar == vpi1, width, is_signed);
            return 1;
        case vpiIntVal:
            *result = sv4_from_i64(input->value.integer, width);
            return 1;
        case vpiVectorVal:
            if (!input->value.vector) return 0;
            {
                uint64_t bits[LLG_LIMBS];
                uint64_t x[LLG_LIMBS];
                uint64_t z[LLG_LIMBS];
                memset(bits, 0, sizeof(bits));
                memset(x, 0, sizeof(x));
                memset(z, 0, sizeof(z));
                for (uint32_t bit = 0; bit < width; ++bit) {
                    uint32_t word = bit / 32u;
                    uint32_t shift = bit % 32u;
                    uint32_t aval = input->value.vector[word].aval;
                    uint32_t bval = input->value.vector[word].bval;
                    if ((aval >> shift) & 1u) bits[bit / 64u] |= 1ULL << (bit % 64u);
                    if ((bval >> shift) & 1u) {
                        if ((aval >> shift) & 1u) x[bit / 64u] |= 1ULL << (bit % 64u);
                        else z[bit / 64u] |= 1ULL << (bit % 64u);
                    }
                }
                *result = sv4_from_limbs(bits, x, z, width, is_signed);
                return 1;
            }
        default: return 0;
    }
}

vpiHandle vpi_put_value(vpiHandle handle, p_vpi_value input, p_vpi_time time,
                        PLI_INT32 flags) {
    if (time || flags != vpiNoDelay) {
        vpi_set_error(vpiRun, vpiError, "LLG_VPI_UNSUPPORTED",
                      "VPI delayed, forced, and scheduled writes are unsupported");
        return NULL;
    }
    if (valid_handle(handle, LLG_VPI_CALL)) {
        llg_vpi_call_t* call = ((llg_vpi_handle_t*)handle)->call;
        if (!call->is_function) {
            vpi_set_error(vpiRun, vpiError, "LLG_VPI_UNSUPPORTED",
                          "system-task handles cannot receive values");
            return NULL;
        }
        if (!call->has_real_return) {
            if (!value_from_vpi(input, call->return_value.width, call->return_value.is_signed, &call->return_value))
                vpi_set_error(vpiRun, vpiError, "LLG_VPI_VALUE", "invalid system-function return value");
        } else if (!input || input->format != vpiRealVal) {
            vpi_set_error(vpiRun, vpiError, "LLG_VPI_VALUE", "real system-function requires vpiRealVal");
        } else {
            call->real_return = input->value.real;
        }
        return handle;
    }
    if (!valid_handle(handle, LLG_VPI_OBJECT)) {
        vpi_set_error(vpiRun, vpiError, "LLG_VPI_HANDLE", "invalid VPI handle in vpi_put_value");
        return NULL;
    }
    llg_vpi_model_object_t* object = ((llg_vpi_handle_t*)handle)->object;
    if (object->is_net || object->is_real || !object->packed) {
        vpi_set_error(vpiRun, vpiError, "LLG_VPI_UNSUPPORTED", "VPI writes are limited to packed variables");
        return NULL;
    }
    sv4_t value;
    if (!value_from_vpi(input, object->width, object->is_signed, &value)) {
        vpi_set_error(vpiRun, vpiError, "LLG_VPI_VALUE", "invalid packed value for VPI write");
        return NULL;
    }
    llg_ba(object->packed, value);
    return handle;
}

void vpi_get_time(vpiHandle object, p_vpi_time time) {
    if (object && !valid_handle(object, LLG_VPI_OBJECT) &&
        !valid_handle(object, LLG_VPI_CALL) &&
        !valid_handle(object, LLG_VPI_ARGUMENT)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI time object handle");
        return;
    }
    if (!time) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_ARGUMENT", "null VPI time output");
        return;
    }
    if (time->type == vpiSuppressTime) return;
    if (time->type != vpiSimTime && time->type != vpiScaledRealTime) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_TIME", "unsupported VPI time format");
        return;
    }
    uint64_t now = llg_time();
    if (time->type == vpiSimTime) {
        time->high = (PLI_UINT32)(now >> 32);
        time->low = (PLI_UINT32)now;
        return;
    }
    uint64_t precision = llg_time_precision_fs();
    uint64_t unit = 0;
    if (valid_handle(object, LLG_VPI_OBJECT)) {
        llg_vpi_model_object_t* scope = ((llg_vpi_handle_t*)object)->object;
        for (; scope && !unit; scope = scope->parent) unit = scope->time_unit_fs;
    } else if (valid_handle(object, LLG_VPI_CALL)) {
        unit = ((llg_vpi_handle_t*)object)->call->time_unit_fs;
    } else if (valid_handle(object, LLG_VPI_ARGUMENT)) {
        llg_vpi_call_t* call = ((llg_vpi_handle_t*)object)->owner_call;
        if (call) unit = call->time_unit_fs;
    }
    if (!precision) precision = 1;
    if (!unit) unit = precision;
    time->real = (double)((long double)now * precision / unit);
}

PLI_INT32 vpi_compare_objects(vpiHandle object1, vpiHandle object2) {
    llg_vpi_handle_t* first = known_handle(object1);
    llg_vpi_handle_t* second = known_handle(object2);
    if (!first || !second || first->magic != LLG_VPI_MAGIC ||
        second->magic != LLG_VPI_MAGIC ||
        first->generation != g_vpi.generation ||
        second->generation != g_vpi.generation) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI object comparison");
        return 0;
    }
    if (valid_handle(object1, LLG_VPI_OBJECT) && valid_handle(object2, LLG_VPI_OBJECT))
        return ((llg_vpi_handle_t*)object1)->object == ((llg_vpi_handle_t*)object2)->object;
    return object1 == object2;
}

PLI_INT32 vpi_chk_error(p_vpi_error_info output) {
    if (!g_vpi.error_pending) return 0;
    if (output) {
        output->state = g_vpi.error_state;
        output->level = g_vpi.error_level;
        output->message = (PLI_BYTE8*)g_vpi.error_message;
        output->product = (PLI_BYTE8*)g_vpi.error_product;
        output->code = (PLI_BYTE8*)g_vpi.error_code;
        output->file = g_vpi.error_file[0] ? (PLI_BYTE8*)g_vpi.error_file : NULL;
        output->line = g_vpi.error_line;
    }
    g_vpi.error_pending = 0;
    return 1;
}

PLI_INT32 vpi_free_object(vpiHandle handle) { return vpi_release_handle(handle); }

PLI_INT32 vpi_release_handle(vpiHandle handle) {
    if (!handle) return 0;
    llg_vpi_handle_t* object = known_handle(handle);
    if (!object || object->magic != LLG_VPI_MAGIC ||
        object->generation != g_vpi.generation) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid or stale VPI release handle");
        return 0;
    }
    if (object->kind == LLG_VPI_OBJECT || object->kind == LLG_VPI_SYSTF) return 1;
    if (object->kind == LLG_VPI_CALLBACK) return vpi_remove_cb(handle);
    destroy_dynamic_handle(object);
    return 1;
}

void* vpi_get_userdata(vpiHandle handle) {
    llg_vpi_handle_t* object = known_handle(handle);
    if (!object || object->magic != LLG_VPI_MAGIC ||
        object->generation != g_vpi.generation) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI userdata handle");
        return NULL;
    }
    return object->userdata;
}

PLI_INT32 vpi_put_userdata(vpiHandle handle, void* userdata) {
    llg_vpi_handle_t* object = known_handle(handle);
    if (!object || object->magic != LLG_VPI_MAGIC ||
        object->generation != g_vpi.generation) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI userdata handle");
        return 0;
    }
    object->userdata = userdata;
    return 1;
}

vpiHandle vpi_register_cb(p_cb_data data) {
    if (!data || !data->cb_rtn || !callback_reason_supported(data->reason) ||
        g_vpi.callback_count >= LLG_VPI_MAX_CALLBACKS) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_CALLBACK", "unsupported or invalid VPI callback registration");
        return NULL;
    }
    llg_vpi_handle_t* handle = make_handle(LLG_VPI_CALLBACK, 0);
    if (!handle) return NULL;
    handle->callback = *data;
    handle->callback.user_data = data->user_data;
    handle->callback.obj = data->obj;
    handle->callback.index = data->index;
    g_vpi.callback_count++;
    handle->next = g_vpi.callbacks;
    g_vpi.callbacks = handle;
    return handle;
}

PLI_INT32 vpi_remove_cb(vpiHandle handle) {
    if (!valid_handle(handle, LLG_VPI_CALLBACK)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI callback handle");
        return 0;
    }
    llg_vpi_handle_t** cursor = &g_vpi.callbacks;
    while (*cursor && *cursor != (llg_vpi_handle_t*)handle) cursor = &(*cursor)->next;
    if (!*cursor) return 0;
    *cursor = (*cursor)->next;
    if (g_vpi.callback_count > 0) --g_vpi.callback_count;
    invalidate_handle((llg_vpi_handle_t*)handle);
    return 1;
}

void vpi_get_cb_info(vpiHandle handle, p_cb_data output) {
    if (!valid_handle(handle, LLG_VPI_CALLBACK)) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_HANDLE", "invalid VPI callback handle");
        return;
    }
    if (!output) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_ARGUMENT", "null VPI callback info output");
        return;
    }
    *output = ((llg_vpi_handle_t*)handle)->callback;
}

static void invoke_callbacks(int reason) {
    for (llg_vpi_handle_t* cursor = g_vpi.callbacks; cursor; cursor = cursor->next) {
        if (!valid_handle(cursor, LLG_VPI_CALLBACK) || cursor->callback.reason != reason) continue;
        s_vpi_time time;
        memset(&time, 0, sizeof(time));
        time.type = vpiSimTime;
        vpi_get_time(NULL, &time);
        s_cb_data data = cursor->callback;
        data.time = &time;
        data.value = NULL;
        if (data.cb_rtn(&data) != 0) {
            g_vpi.failed = 1;
            vpi_set_error(vpiRun, vpiError, "LLG_VPI_CALLBACK", "VPI callback returned failure");
            llg_rt_request_finish();
        }
    }
}

void llg_vpi_start_simulation(void) {
    if (!g_vpi.objects || g_vpi.failed) return;
    g_vpi.running = 1;
    invoke_callbacks(cbStartOfSimulation);
}

void llg_vpi_end_simulation(void) {
    if (!g_vpi.objects) return;
    invoke_callbacks(cbEndOfSimulation);
    g_vpi.running = 0;
}

static void make_compile_call(llg_vpi_call_t* call, const char* name,
                              const llg_vpi_compile_arg_t* args, int count) {
    memset(call, 0, sizeof(*call));
    call->name = name;
    call->arg_count = count;
    call->args = (llg_vpi_arg_t*)calloc((size_t)count, sizeof(*call->args));
    if (!call->args && count) return;
    for (int i = 0; i < count; ++i) {
        call->args[i].width = args[i].width;
        call->args[i].is_signed = args[i].is_signed;
        call->args[i].is_real = args[i].is_real;
        call->args[i].kind = args[i].is_real ? LLG_FMT_REAL : LLG_FMT_PACKED;
        call->args[i].packed = args[i].is_real ? sv4_x(1, 0) : sv4_x(args[i].width ? args[i].width : 1, args[i].is_signed);
    }
}

static void release_compile_call(llg_vpi_call_t* call) {
    if (call) free(call->args);
}

static void invoke_compiletf(llg_vpi_call_t* call) {
    llg_vpi_registration_t* registration = find_registration(call->name);
    if (!registration) {
        vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_UNRESOLVED", "no VPI registration for `%s`", call->name);
        return;
    }
    call->registration = registration;
    call->is_function = registration_type_is_function(registration);
    call->has_real_return = registration->data.sysfunctype == vpiRealFunc;
    call->return_value = sv4_x(
        32,
        registration->data.sysfunctype == vpiSizedSignedFunc ||
            registration->data.sysfunctype == vpiIntFunc);
    if (registration->data.compiletf) {
        g_vpi.active_call = call;
        llg_vpi_handle_t* handle = call_handle();
        PLI_INT32 result = registration->data.compiletf(registration->data.user_data);
        vpi_release_handle(handle);
        invalidate_call_handles(call);
        g_vpi.active_call = NULL;
        if (result != 0) {
            vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_COMPILETf", "compiletf rejected `%s`", call->name);
        }
    }
    if (!g_vpi.error_pending && registration->data.sizetf && registration_type_is_function(registration) &&
        (registration->data.sysfunctype == vpiSizedFunc || registration->data.sysfunctype == vpiSizedSignedFunc)) {
        g_vpi.active_call = call;
        llg_vpi_handle_t* handle = call_handle();
        PLI_INT32 width = registration->data.sizetf(registration->data.user_data);
        vpi_release_handle(handle);
        // sizetf can create borrowed argument and iterator handles too.
        invalidate_call_handles(call);
        g_vpi.active_call = NULL;
        if (width <= 0 || (uint32_t)width > LLG_MAX_WIDTH) {
            vpi_set_errorf(vpiCompile, vpiError, "LLG_VPI_SIZETF", "sizetf for `%s` returned invalid width %d", call->name, width);
        } else {
            call->return_value = sv4_x((uint32_t)width, registration->data.sysfunctype == vpiSizedSignedFunc);
        }
    }

}

int llg_vpi_compile_call(const char* name, const llg_vpi_compile_arg_t* args, int count) {
    if (!name || count < 0 || count > LLG_VPI_MAX_ARGS || (count && !args)) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_ARGUMENT", "invalid VPI compile-call descriptor");
        return 0;
    }
    llg_vpi_call_t call;
    make_compile_call(&call, name, args, count);
    if (count && !call.args) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI compile-call arguments allocation failed");
        return 0;
    }
    invoke_compiletf(&call);
    release_compile_call(&call);
    return !g_vpi.error_pending;
}

static llg_vpi_callsite_t* find_callsite(uint64_t id) {
    for (llg_vpi_callsite_t* site = g_vpi.callsites; site; site = site->next)
        if (site->id == id) return site;
    return NULL;
}

int llg_vpi_compile_call_site(uint64_t id, const char* name,
    const llg_vpi_compile_arg_t* args, int count, uint64_t time_unit_fs) {
    if (id == UINT64_MAX || find_callsite(id) || !name || count < 0 ||
        count > LLG_VPI_MAX_ARGS || (count && !args) ||
        g_vpi.callsite_count >= LLG_VPI_MAX_HANDLES) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_CALLSITE", "invalid or duplicate VPI callsite");
        return 0;
    }
    llg_vpi_callsite_t* site = (llg_vpi_callsite_t*)calloc(1, sizeof(*site));
    if (!site) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI callsite allocation failed");
        return 0;
    }
    if (count) {
        site->args = (llg_vpi_compile_arg_t*)malloc((size_t)count * sizeof(*args));
        if (!site->args) {
            free(site);
            vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI argument shape allocation failed");
            return 0;
        }
        memcpy(site->args, args, (size_t)count * sizeof(*args));
    }
    site->id = id;
    site->time_unit_fs = time_unit_fs;
    site->arg_count = count;
    site->next = g_vpi.callsites;
    g_vpi.callsites = site;
    g_vpi.callsite_count++;
    llg_vpi_call_t call;
    make_compile_call(&call, name, args, count);
    call.time_unit_fs = time_unit_fs;
    if (count && !call.args) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_NOMEM", "VPI compile-call allocation failed");
        return 0;
    }
    invoke_compiletf(&call);
    site->registration = call.registration;
    site->result_real = (int8_t)call.has_real_return;
    site->result_width = call.has_real_return ? 0 : call.return_value.width;
    site->result_signed = call.return_value.is_signed;
    site->valid = !g_vpi.error_pending && call.registration != NULL;
    release_compile_call(&call);
    return site->valid;
}

int llg_vpi_compile_calls(const char* const* names,
                          const llg_vpi_compile_arg_t* const* args,
                          const int* counts, int call_count) {
    if (call_count < 0 || call_count > LLG_VPI_MAX_ARGS || (call_count && (!names || !counts))) {
        vpi_set_error(vpiCompile, vpiError, "LLG_VPI_ARGUMENT", "invalid VPI compile-call list");
        return 0;
    }
    for (int i = 0; i < call_count; ++i) {
        if (!llg_vpi_compile_call(names[i], args ? args[i] : NULL, counts[i])) return 0;
    }
    return 1;
}

static int prepare_runtime_call(llg_vpi_call_t* call, uint64_t id, const char* name,
                                llg_vpi_arg_t* args, int count, int function) {
    llg_vpi_registration_t* registration = find_registration(name);
    if (!registration || (function != registration_type_is_function(registration))) {
        vpi_set_errorf(vpiRun, vpiError, "LLG_VPI_UNRESOLVED", "unregistered VPI %s `%s`", function ? "function" : "task", name ? name : "<null>");
        return 0;
    }
    if (count < 0 || count > LLG_VPI_MAX_ARGS || (count && !args)) {
        vpi_set_error(vpiRun, vpiError, "LLG_VPI_ARGUMENT", "invalid VPI call arguments");
        return 0;
    }
    memset(call, 0, sizeof(*call));
    call->name = name;
    call->args = args;
    call->arg_count = count;
    call->registration = registration;
    call->is_function = function;
    if (id == UINT64_MAX) {
        // Legacy embedding entry points lack source identity. Recompute this
        // invocation's descriptor rather than reuse another call's return size.
        invoke_compiletf(call);
        return !g_vpi.error_pending;
    }
    llg_vpi_callsite_t* site = find_callsite(id);
    if (!site || !site->valid || site->registration != registration || site->arg_count != count) {
        vpi_set_error(vpiRun, vpiError, "LLG_VPI_CALLSITE", "uncompiled or inconsistent VPI callsite");
        return 0;
    }
    for (int i = 0; i < count; i++) {
        if (site->args[i].width != args[i].width ||
            site->args[i].is_signed != args[i].is_signed ||
            site->args[i].is_real != args[i].is_real) {
            vpi_set_error(vpiRun, vpiError, "LLG_VPI_CALLSITE", "VPI argument shape changed after compilation");
            return 0;
        }
    }
    call->time_unit_fs = site->time_unit_fs;
    call->has_real_return = site->result_real;
    call->return_value = sv4_x(site->result_width ? site->result_width : 1, site->result_signed);
    return 1;
}

static int invoke_calltf(llg_vpi_call_t* call) {
    g_vpi.active_call = call;
    llg_vpi_handle_t* handle = call_handle();
    PLI_INT32 result = call->registration->data.calltf(call->registration->data.user_data);
    vpi_release_handle(handle);
    invalidate_call_handles(call);
    g_vpi.active_call = NULL;
    if (result != 0) {
        vpi_fail_runtime("VPI calltf returned failure");
        return 0;
    }
    return 1;
}

int llg_vpi_call_task_site(uint64_t site, const char* name, llg_vpi_arg_t* args, int count) {
    llg_vpi_call_t call;
    if (!prepare_runtime_call(&call, site, name, args, count, 0)) return 0;
    return invoke_calltf(&call);
}

sv4_t llg_vpi_call_function_site(uint64_t site, const char* name, llg_vpi_arg_t* args, int count,
                            uint32_t fallback_width, int8_t fallback_signed) {
    llg_vpi_call_t call;
    if (!prepare_runtime_call(&call, site, name, args, count, 1)) {
        return sv4_x(fallback_width ? fallback_width : 1, fallback_signed);
    }
    if (call.has_real_return) {
        vpi_fail_runtime("VPI real function used in a packed expression");
        return sv4_x(fallback_width ? fallback_width : 1, fallback_signed);
    }
    if (!invoke_calltf(&call)) return call.return_value;
    return call.return_value;
}

double llg_vpi_call_real_function_site(uint64_t site, const char* name, llg_vpi_arg_t* args, int count) {
    llg_vpi_call_t call;
    if (!prepare_runtime_call(&call, site, name, args, count, 1)) return 0.0;
    if (!call.has_real_return) {
        vpi_fail_runtime("VPI packed function used in a real expression");
        return 0.0;
    }
    if (!invoke_calltf(&call)) return 0.0;
    return call.real_return;
}

int llg_vpi_call_task(const char* name, llg_vpi_arg_t* args, int count) {
    return llg_vpi_call_task_site(UINT64_MAX, name, args, count);
}
sv4_t llg_vpi_call_function(const char* name, llg_vpi_arg_t* args, int count,
                          uint32_t width, int8_t sign) {
    return llg_vpi_call_function_site(UINT64_MAX, name, args, count, width, sign);
}
double llg_vpi_call_real_function(const char* name, llg_vpi_arg_t* args, int count) {
    return llg_vpi_call_real_function_site(UINT64_MAX, name, args, count);
}

int llg_vpi_failed(void) { return g_vpi.failed || g_vpi.error_pending; }

void llg_vpi_shutdown(void) {
    if (g_vpi.running) llg_vpi_end_simulation();
    llg_vpi_handle_t* cursor = g_vpi.all_handles;
    while (cursor) {
        llg_vpi_handle_t* next = cursor->all_next;
        free(cursor->items);
        cursor->items = NULL;
        invalidate_handle(cursor);
        free(cursor);
        cursor = next;
    }
    g_vpi.all_handles = NULL;
    g_vpi.callbacks = NULL;
    for (int i = 0; i < g_vpi.registration_count; ++i) free(g_vpi.registrations[i].name);
    free(g_vpi.object_handles);
    while (g_vpi.callsites) {
        llg_vpi_callsite_t* site = g_vpi.callsites;
        g_vpi.callsites = site->next;
        free(site->args);
        free(site);
    }
    free(g_vpi.value_vector);
#if defined(_WIN32)
    for (int i = 0; i < g_vpi.plugin_count; ++i) FreeLibrary(g_vpi.plugin_handles[i]);
#else
    for (int i = 0; i < g_vpi.plugin_count; ++i) dlclose(g_vpi.plugin_handles[i]);
#endif
    memset(&g_vpi, 0, sizeof(g_vpi));
}

PLI_INT32 vpi_get_vlog_info(void* info) {
    (void)info;
    vpi_set_error(vpiPLI, vpiError, "LLG_VPI_UNSUPPORTED", "vpi_get_vlog_info is not part of the bounded API");
    return 0;
}

PLI_INT32 vpi_printf(PLI_BYTE8* format, ...) {
    if (!format) {
        vpi_set_error(vpiPLI, vpiError, "LLG_VPI_ARGUMENT", "null vpi_printf format");
        return 0;
    }
    va_list ap;
    va_start(ap, format);
    int result = vfprintf(stdout, (const char*)format, ap);
    va_end(ap);
    return result;
}

PLI_INT32 vpi_flush(void) { return fflush(stdout) == 0; }

PLI_INT32 vpi_control(PLI_INT32 operation, ...) {
    switch (operation) {
        case vpiFinish:
            llg_rt_request_finish();
            return 1;
        case vpiStop:
            llg_rt_stop();
            return 1;
        default:
            vpi_set_error(vpiRun, vpiError, "LLG_VPI_UNSUPPORTED", "unsupported vpi_control operation");
            return 0;
    }
}
