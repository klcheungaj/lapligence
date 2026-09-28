#ifndef LLG_SELFTEST_CO_H
#define LLG_SELFTEST_CO_H

// Compact explicit frame used by hand-written runtime scheduler tests. Tests
// use only the fields they need across a suspension; arm inputs that are
// copied synchronously remain compound literals in the call expression.
typedef struct {
    llg_co_frame_t co;
    llg_fork_group_t* group;
    llg_frame_t* capture;
    llg_activation_t* activation;
    uint64_t time;
    int index;
    int flag;
} llg_selftest_frame_t;
LLG_CO_ROOT_FRAME_OK(llg_selftest_frame_t);

#define LLG_SELFTEST_CASES_0
#define LLG_SELFTEST_CASES_1 LLG_CO_RESUME_CASE(1)
#define LLG_SELFTEST_CASES_2 LLG_SELFTEST_CASES_1 LLG_CO_RESUME_CASE(2)
#define LLG_SELFTEST_CASES_3 LLG_SELFTEST_CASES_2 LLG_CO_RESUME_CASE(3)
#define LLG_SELFTEST_CASES_4 LLG_SELFTEST_CASES_3 LLG_CO_RESUME_CASE(4)
#define LLG_SELFTEST_CASES_5 LLG_SELFTEST_CASES_4 LLG_CO_RESUME_CASE(5)
#define LLG_SELFTEST_CASES_6 LLG_SELFTEST_CASES_5 LLG_CO_RESUME_CASE(6)
#define LLG_SELFTEST_CASES_7 LLG_SELFTEST_CASES_6 LLG_CO_RESUME_CASE(7)
#define LLG_SELFTEST_CASES_8 LLG_SELFTEST_CASES_7 LLG_CO_RESUME_CASE(8)

#define LLG_SELFTEST_PROCESS(name, waits)                                  \
    static llg_co_status_t name(llg_co_frame_t*, llg_co_chain_t*);         \
    static const llg_co_site_t name##_sites[(waits) + 1] = {{0}};          \
    static const llg_co_desc_t name##_desc = {                             \
        name, #name, sizeof(llg_selftest_frame_t), name##_sites,           \
        (waits) + 1, 0};                                                   \
    static llg_co_status_t name(llg_co_frame_t* co, llg_co_chain_t* ch)

#define LLG_SELFTEST_BEGIN(waits)                                          \
    llg_selftest_frame_t* F = (llg_selftest_frame_t*)co;                   \
    llg_proc_t* self = LLG_CO_OWNER(ch, llg_proc_t);                       \
    (void)F;                                                               \
    (void)self;                                                            \
    LLG_CO_DISPATCH_BEGIN(co)                                              \
    LLG_SELFTEST_CASES_##waits                                             \
    LLG_CO_DISPATCH_END(co)

#define LLG_SELFTEST_AWAIT(site, arm) LLG_CO_AWAIT(co, ch, site, arm)
#define LLG_SELFTEST_DONE() return LLG_CO_DONE
#define LLG_SELFTEST_FINISH()                                              \
    do {                                                                   \
        llg_rt_finish();                                                   \
        return LLG_CO_EXIT;                                                \
    } while (0)

#endif
