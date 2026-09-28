#ifndef LLG_PROBE_CO_H
#define LLG_PROBE_CO_H

// Explicit-frame boilerplate shared by the hand-written runtime probes. A
// probe declares its own POD frame when values must survive a suspension.
typedef struct {
    llg_co_frame_t co;
    uint64_t time;
    int index;
    int flag;
} llg_probe_frame_t;
LLG_CO_ROOT_FRAME_OK(llg_probe_frame_t);

#define LLG_PROBE_CASES_0
#define LLG_PROBE_CASES_1 LLG_CO_RESUME_CASE(1)
#define LLG_PROBE_CASES_2 LLG_PROBE_CASES_1 LLG_CO_RESUME_CASE(2)
#define LLG_PROBE_CASES_3 LLG_PROBE_CASES_2 LLG_CO_RESUME_CASE(3)
#define LLG_PROBE_CASES_4 LLG_PROBE_CASES_3 LLG_CO_RESUME_CASE(4)
#define LLG_PROBE_CASES_5 LLG_PROBE_CASES_4 LLG_CO_RESUME_CASE(5)
#define LLG_PROBE_CASES_6 LLG_PROBE_CASES_5 LLG_CO_RESUME_CASE(6)
#define LLG_PROBE_CASES_7 LLG_PROBE_CASES_6 LLG_CO_RESUME_CASE(7)
#define LLG_PROBE_CASES_8 LLG_PROBE_CASES_7 LLG_CO_RESUME_CASE(8)

#define LLG_PROBE_PROCESS(name, frame_type, waits)                         \
    static llg_co_status_t name(llg_co_frame_t*, llg_co_chain_t*);         \
    static const llg_co_site_t name##_sites[(waits) + 1] = {{0}};          \
    static const llg_co_desc_t name##_desc = {                             \
        name, #name, sizeof(frame_type), name##_sites, (waits) + 1, 0};    \
    static llg_co_status_t name(llg_co_frame_t* co, llg_co_chain_t* ch)

#define LLG_PROBE_SIMPLE_PROCESS(name, waits)                              \
    LLG_PROBE_PROCESS(name, llg_probe_frame_t, waits)

#define LLG_PROBE_BEGIN(frame_type, waits)                                 \
    frame_type* F = (frame_type*)co;                                       \
    llg_proc_t* self = LLG_CO_OWNER(ch, llg_proc_t);                       \
    (void)F;                                                               \
    (void)self;                                                            \
    LLG_CO_DISPATCH_BEGIN(co)                                              \
    LLG_PROBE_CASES_##waits                                                \
    LLG_CO_DISPATCH_END(co)

#define LLG_PROBE_SIMPLE_BEGIN(waits) LLG_PROBE_BEGIN(llg_probe_frame_t, waits)
#define LLG_PROBE_AWAIT(site, arm) LLG_CO_AWAIT(co, ch, site, arm)
#define LLG_PROBE_DONE() return LLG_CO_DONE
#define LLG_PROBE_EXIT() return LLG_CO_EXIT

#endif
