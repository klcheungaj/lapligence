// llg_vpi.h — generated-model bridge for the bounded public VPI surface.
#ifndef LLG_VPI_H
#define LLG_VPI_H

#include "llg_rt.h"
#include "vpi_user.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct llg_vpi_model_object {
    int type;
    const char* name;
    const char* full_name;
    const char* definition_name;
    const char* file;
    int line;
    uint32_t width;
    int8_t is_signed;
    int8_t is_real;
    int8_t is_net;
    sv4_t* packed;
    double* real;
    struct llg_vpi_model_object* parent;
} llg_vpi_model_object_t;

typedef struct {
    uint32_t width;
    int8_t is_signed;
    int8_t is_real;
} llg_vpi_compile_arg_t;

typedef struct {
    int kind;
    uint32_t width;
    int8_t is_signed;
    int8_t is_real;
    sv4_t packed;
    double real;
} llg_vpi_arg_t;

/* Model metadata is static generated storage; the runtime borrows it until
 * llg_vpi_shutdown, while all public handles remain generation-checked. */
int llg_vpi_model_init(const char* design_name,
                       llg_vpi_model_object_t* objects, size_t object_count);
int llg_vpi_startup(void);
int llg_vpi_compile_call(const char* name, const llg_vpi_compile_arg_t* args,
                         int arg_count);
int llg_vpi_compile_calls(const char* const* names,
                          const llg_vpi_compile_arg_t* const* args,
                          const int* arg_counts, int call_count);
void llg_vpi_start_simulation(void);
void llg_vpi_end_simulation(void);
void llg_vpi_shutdown(void);

int llg_vpi_call_task(const char* name, llg_vpi_arg_t* args, int arg_count);
sv4_t llg_vpi_call_function(const char* name, llg_vpi_arg_t* args,
                            int arg_count, uint32_t fallback_width,
                            int8_t fallback_signed);
double llg_vpi_call_real_function(const char* name, llg_vpi_arg_t* args,
                                  int arg_count);
int llg_vpi_failed(void);

#ifdef __cplusplus
}
#endif

#endif /* LLG_VPI_H */
