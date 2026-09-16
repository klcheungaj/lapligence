// llg_wave.h — asynchronous VCD/FST waveform support for generated models.
#ifndef LLG_WAVE_H
#define LLG_WAVE_H

#include "llg_rt.h"

#ifdef __cplusplus
extern "C" {
#endif

int llg_wave_model_init(uint64_t precision_fs);
int llg_wave_register_sv4(const char* hierarchical_name, sv4_t* value,
                          uint32_t width);
int llg_wave_register_real(const char* hierarchical_name, double* value);
void llg_wave_file(const char* path, uint64_t now);
void llg_wave_dumpvars(uint64_t now);
void llg_wave_dumpvars_select(uint64_t now, uint32_t depth,
                              const char* const* names, uint32_t name_count);
void llg_wave_on(uint64_t now);
void llg_wave_off(uint64_t now);
void llg_wave_dumpall(uint64_t now);
void llg_wave_flush(uint64_t now);
void llg_wave_limit(uint64_t bytes, uint64_t now);
int llg_wave_close(uint64_t now);

// Runtime-internal hooks. The packed input is borrowed only for this call and
// deep-copied before return. The queue/consumer own the snapshot allocation;
// the writer thread never reads mutable model storage or borrowed limb arrays.
void llg_wave_changed_sv4(sv4_t* ptr, const sv4_t* value, uint64_t now);
void llg_wave_changed_real(double* ptr, double value, uint64_t now);

#ifdef __cplusplus
}
#endif

#endif // LLG_WAVE_H
