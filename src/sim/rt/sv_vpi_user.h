/*
 * sv_vpi_user.h — the SystemVerilog VPI extensions a generated Lapligence
 * model supports: the assertion and coverage subset of IEEE 1800-2009
 * Annex M, with the standard constant values. It includes vpi_user.h, which
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

#endif /* SV_VPI_USER_H */
