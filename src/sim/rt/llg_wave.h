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
// Registrations whose scopes are not all modules: `scopes` has one letter per
// scope of `hierarchical_name`, outermost first (`m` module or generate scope,
// `b` begin block, `k` fork block, `t` task, `f` function; `B` and `K` for an
// unnamed block, whose generated `unnamed$$_N` name is written as it stands).
// The string must outlive the model; the writer reads it for the header.
int llg_wave_register_scoped_sv4(const char* hierarchical_name, const char* scopes,
                                 sv4_t* value, uint32_t width);
int llg_wave_register_scoped_real(const char* hierarchical_name, const char* scopes,
                                  double* value);
// A named event: `$var event`, with a change in each time step that triggers
// its synchronization object. Later handle assignments do not move the
// registration; it follows the object declared with the event.
int llg_wave_register_event(const char* hierarchical_name, const char* scopes,
                            llg_event_object_t* event);
void llg_wave_file(const char* path, uint64_t now);
// `$dumpfile` of a computed name: `len` bytes, not terminated, copied here.
void llg_wave_file_bytes(const char* path, size_t len, uint64_t now);
void llg_wave_dumpvars(uint64_t now);
void llg_wave_dumpvars_select(uint64_t now, uint32_t depth,
                              const char* const* names, uint32_t name_count);
// Driver-requested dumping (`llg --wave`): open `path` (unless
// LLG_SIM_WAVE_FILE replaces it) and select like $dumpvars at time zero. The
// design's later $dumpfile/$dumpvars calls are ignored; $dumpon/$dumpoff,
// $dumpall, $dumpflush and $dumplimit keep working. Returns 0 on success.
int llg_wave_start(const char* path, uint32_t depth, const char* const* names,
                   uint32_t name_count);
void llg_wave_on(uint64_t now);
void llg_wave_off(uint64_t now);
void llg_wave_dumpall(uint64_t now);
void llg_wave_flush(uint64_t now);
void llg_wave_limit(uint64_t bytes, uint64_t now);
int llg_wave_close(uint64_t now);

// End of the current time step (the scheduler calls it before advancing
// time; close calls it for the last step): starts a pending $dumpvars dump or
// records the step's final value of every changed address.
void llg_wave_slot_end(uint64_t now);

// Runtime-internal hooks, called after a store changed the registered
// storage at `ptr`. They only mark the address; llg_wave_slot_end copies its
// value, on the simulation thread, into an event the writer thread owns, so
// the writer never reads mutable model storage.
void llg_wave_changed_sv4(const sv4_t* ptr);
void llg_wave_changed_real(const double* ptr);
void llg_wave_changed_event(const llg_event_object_t* ptr);

#ifdef __cplusplus
}
#endif

#endif // LLG_WAVE_H
