/*
 * sv_vpi_user.h — the SystemVerilog VPI extensions a generated Lapligence
 * model supports: the assertion and coverage subset of IEEE 1800-2009
 * Annex M, with the standard constant values, and the Annex M routine
 * vpi_register_assertion_cb(). It includes vpi_user.h, which
 * already carries the Annex M object types of the catalog (vpiInterface,
 * the SystemVerilog variable types).
 */
#ifndef SV_VPI_USER_H
#define SV_VPI_USER_H

#include "vpi_user.h"

/* Assertion object types (39.3.2). */
#define vpiAssert 686
#define vpiAssume 687
#define vpiCover 688
#define vpiImmediateAssert 665
#define vpiImmediateAssume 694
#define vpiImmediateCover 695
#define vpiAssertion 744

/* Assertion properties. */
#define vpiIsDeferred 657

/* Coverage control (40.5.2). */
#define vpiCoverageStart 750
#define vpiCoverageStop 751
#define vpiCoverageReset 752
#define vpiCoverageCheck 753
#define vpiCoverageMerge 754
#define vpiCoverageSave 755

/* Coverage type properties. Statement, toggle and FSM coverage are not
 * implemented: they fail through vpi_chk_error(). */
#define vpiAssertCoverage 760
#define vpiFsmStateCoverage 761
#define vpiStatementCoverage 762
#define vpiToggleCoverage 763

/* Coverage status properties. */
#define vpiCovered 765
#define vpiCoverMax 766
#define vpiCoveredCount 767

/* Assertion-specific coverage status properties. */
#define vpiAssertAttemptCovered 770
#define vpiAssertSuccessCovered 771
#define vpiAssertFailureCovered 772
#define vpiAssertVacuousSuccessCovered 773
#define vpiAssertDisableCovered 774
#define vpiAssertKillCovered 777

/* The $coverage_control status values of 40.3.1, which vpi_control returns
 * for the coverage operations (an llg addition: Annex M defines them only
 * as SystemVerilog macros). */
#define SV_COV_OVERFLOW (-2)
#define SV_COV_ERROR (-1)
#define SV_COV_NOCOV 0
#define SV_COV_OK 1
#define SV_COV_PARTIAL 2

/* Assertion callbacks (39.4.2). The routine is declared and defined so that
 * applications link; llg does not run assertion callbacks yet, so it returns
 * NULL with a vpi_chk_error() error (LLG_VPI_UNSUPPORTED). */
/* Assertion callback reasons (Annex M), the `reason` of
 * vpi_register_assertion_cb(). */
#define cbAssertionStart 606
#define cbAssertionSuccess 607
#define cbAssertionFailure 608
#define cbAssertionVacuousSuccess 657
#define cbAssertionDisabledEvaluation 658
#define cbAssertionStepSuccess 609
#define cbAssertionStepFailure 610
#define cbAssertionDisable 611
#define cbAssertionEnable 612
#define cbAssertionReset 613
#define cbAssertionKill 614
#define cbAssertionEnablePassAction 645
#define cbAssertionEnableFailAction 646
#define cbAssertionDisablePassAction 647
#define cbAssertionDisableFailAction 648
#define cbAssertionEnableNonvacuousAction 649
#define cbAssertionDisableVacuousAction 650
#define cbAssertionSysInitialized 615
#define cbAssertionSysOn 616
#define cbAssertionSysOff 617
#define cbAssertionSysKill 631
#define cbAssertionSysEnd 618
#define cbAssertionSysReset 619
#define cbAssertionSysEnablePassAction 651
#define cbAssertionSysEnableFailAction 652
#define cbAssertionSysDisablePassAction 653
#define cbAssertionSysDisableFailAction 654
#define cbAssertionSysEnableNonvacuousAction 655
#define cbAssertionSysDisableVacuousAction 656

typedef struct t_vpi_assertion_step_info {
    PLI_INT32 matched_expression_count;
    vpiHandle* matched_exprs;
    PLI_INT32 stateFrom, stateTo;
} s_vpi_assertion_step_info, *p_vpi_assertion_step_info;

typedef struct t_vpi_attempt_info {
    union {
        vpiHandle failExpr;
        p_vpi_assertion_step_info step;
    } detail;
    s_vpi_time attemptStartTime;
} s_vpi_attempt_info, *p_vpi_attempt_info;

typedef PLI_INT32(vpi_assertion_callback_func)(PLI_INT32 reason,
                                               p_vpi_time cb_time,
                                               vpiHandle assertion,
                                               p_vpi_attempt_info info,
                                               PLI_BYTE8* user_data);

#ifdef __cplusplus
extern "C" {
#endif

vpiHandle vpi_register_assertion_cb(vpiHandle assertion, PLI_INT32 reason,
                                    vpi_assertion_callback_func* cb_rtn,
                                    PLI_BYTE8* user_data);

#ifdef __cplusplus
}
#endif

#endif /* SV_VPI_USER_H */
