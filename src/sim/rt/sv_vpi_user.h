/*
 * sv_vpi_user.h — the SystemVerilog VPI extensions a generated Lapligence
 * model supports: the assertion and coverage subset of IEEE 1800-2009
 * Annex M, the typespec, structure-member and enum-constant objects, with
 * the standard constant values, the assertion callback and control
 * constants, the sequence and property objects, and the Annex M routine
 * vpi_register_assertion_cb(). It
 * includes vpi_user.h, which already carries the Annex M object types of
 * the catalog (vpiInterface, the SystemVerilog variable types).
 */
#ifndef SV_VPI_USER_H
#define SV_VPI_USER_H

#include "vpi_user.h"

/* Typespecs, members and enum constants (37.17, 37.23, 37.24). */
#define vpiTypespec 605
#define vpiLogicVar vpiReg
#define vpiArrayVar vpiRegArray
#define vpiVarBit vpiRegBit
#define vpiStringVar 616
#define vpiLongIntTypespec 625
#define vpiShortRealTypespec 626
#define vpiByteTypespec 627
#define vpiShortIntTypespec 628
#define vpiIntTypespec 629
#define vpiClassTypespec 630
#define vpiStringTypespec 631
#define vpiEnumTypespec 633
#define vpiEnumConst 634
#define vpiIntegerTypespec 635
#define vpiTimeTypespec 636
#define vpiRealTypespec 637
#define vpiStructTypespec 638
#define vpiUnionTypespec 639
#define vpiBitTypespec 640
#define vpiLogicTypespec 641
#define vpiArrayTypespec 642
#define vpiVoidTypespec 643
#define vpiTypespecMember 644
#define vpiPackedArrayTypespec 692
#define vpiTypedefAlias 701
#define vpiIndexTypespec 702
#define vpiBaseTypespec 703
#define vpiElemTypespec 704
#define vpiTypedef 725
#define vpiMember 742
#define vpiElement 743
#define vpiStructUnionMember 615
#define vpiPacked 630
#define vpiTagged 632

/* Instances, interface ports, processes and task/function properties
 * (37.6-37.15, 37.37, 37.58). */
#define vpiProgram 602
#define vpiInterfaceArray 603
#define vpiProgramArray 604
#define vpiModport 606
#define vpiInterfaceTfDecl 607
#define vpiRefObj 608
#define vpiFinal 676
#define vpiActual 700
#define vpiInstance 745
#define vpiPortType 611
#define vpiInterfacePort 1
#define vpiModportPort 2
#define vpiVisibility 620
#define vpiPublicVis 1
#define vpiAlwaysType 624
#define vpiAlwaysComb 2
#define vpiAlwaysFF 3
#define vpiAlwaysLatch 4
#define vpiMethod 645
#define vpiOtherFunc 6
#define vpiWaitingProcesses 734

/* Dynamic objects: strings, chandles, class definitions, variables and
 * objects, dynamic arrays, queues and associative arrays (37.3.7, 37.17,
 * 37.27-37.29), and their callback reasons (38.36.1). */
#define vpiClassVar 615
#define vpiClassObj 621
#define vpiChandleVar 622
#define vpiChandleTypespec 632
#define vpiClassDefn 652
#define vpiExtends 677
#define vpiDerivedClasses 727
#define vpiMethods 730
#define vpiDynamicArray 2
#define vpiAssocArray 3
#define vpiQueueArray 4
#define vpiClassType 640
#define vpiMailboxClass 1
#define vpiSemaphoreClass 2
#define vpiUserDefinedClass 3
#define vpiAllocScheme 658
#define vpiAutomaticScheme 1
#define vpiDynamicScheme 2
#define vpiOtherScheme 3
#define vpiObjId 660
#define cbSizeChange 605
#define cbCreateObj 700
#define cbReclaimObj 701
#define cbEndOfObject 702
/* Statement and expression objects (37.55-37.73). */
#define vpiReturn 666
#define vpiDoWhile 670
#define vpiOrderedWait 671
#define vpiWaitFork 672
#define vpiDisableFork 673
#define vpiForeachStmt 675
#define vpiReturnStmt 691
#define vpiBreak 684
#define vpiContinue 685
#define vpiMethodFuncCall 648
#define vpiMethodTaskCall 649
#define vpiLoopVars 737
#define vpiLocalVarDecls 609
#define vpiNullConst 11
#define vpiUnboundedConst 10
#define vpiQualifier 650
#define vpiNoQualifier 0
#define vpiUniqueQualifier 1
#define vpiPriorityQualifier 2
#define vpiInsideQualifier 16
#define vpiImplyOp 50
#define vpiPostIncOp 62
#define vpiPreIncOp 63
#define vpiPostDecOp 64
#define vpiPreDecOp 65
#define vpiCastOp 67
#define vpiIffOp 68
#define vpiWildEqOp 69
#define vpiWildNeqOp 70
#define vpiStreamLROp 71
#define vpiStreamRLOp 72
#define vpiAssignmentPatternOp 75
#define vpiMultiAssignmentPatternOp 76
#define vpiInsideOp 95
#define vpiAssignmentOp 82

/* Assertion object types (39.3.2). */
#define vpiAssert 686
#define vpiAssume 687
#define vpiCover 688
#define vpiImmediateAssert 665
#define vpiImmediateAssume 694
#define vpiImmediateCover 695
#define vpiAssertion 744

/* Assertion properties and relations (37.45, 37.46, 39.3.2). */
#define vpiIsDeferred 657
#define vpiDisableCondition 689
#define vpiClockingEvent 690
#define vpiProperty 718
#define vpiStartLine 661
#define vpiColumn 662
#define vpiEndLine 663
#define vpiEndColumn 664
#define vpiIsClockInferred 649
#define vpiIsCoverSequence 659

/* Sequence and property objects (37.47-37.50). */
#define vpiPropertyDecl 655
#define vpiPropertySpec 656
#define vpiPropertyExpr 657
#define vpiPropertyInst 660
#define vpiSequenceDecl 661
#define vpiSequenceInst 664
#define vpiSeqFormalDecl 679
#define vpiPropFormalDecl 699

/* Assertion control (39.5.1, 39.5.2). Step callbacks are not supported:
 * vpiAssertionEnableStep fails through vpi_chk_error(). */
#define vpiAssertionDisable 620
#define vpiAssertionEnable 621
#define vpiAssertionReset 622
#define vpiAssertionKill 623
#define vpiAssertionEnableStep 624
#define vpiAssertionDisableStep 625
#define vpiAssertionClockSteps 626
#define vpiAssertionSysOn 627
#define vpiAssertionSysOff 628
#define vpiAssertionSysKill 632
#define vpiAssertionSysEnd 629
#define vpiAssertionSysReset 630
#define vpiAssertionDisablePassAction 633
#define vpiAssertionEnablePassAction 634
#define vpiAssertionDisableFailAction 635
#define vpiAssertionEnableFailAction 636
#define vpiAssertionDisableVacuousAction 637
#define vpiAssertionEnableNonvacuousAction 638
#define vpiAssertionSysEnablePassAction 639
#define vpiAssertionSysEnableFailAction 640
#define vpiAssertionSysDisablePassAction 641
#define vpiAssertionSysDisableFailAction 642
#define vpiAssertionSysEnableNonvacuousAction 643
#define vpiAssertionSysDisableVacuousAction 644

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

/* Assertion callback reasons (Annex M): the `reason` of
 * vpi_register_assertion_cb() (39.4.2) and, for the cbAssertionSys*
 * reasons, of vpi_register_cb() (39.4.1). The step reasons are refused
 * through vpi_chk_error(): llg has no step callbacks. */
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

/* Collections of handles. IEEE 1800-2009 39.5.1 lets the assertion system
 * controls take "a vpiCollection of handles for a list of scopes" but
 * defines neither the constant nor a constructor; llg takes both from IEEE
 * P1800-2005 30.4.2: vpi_create(vpiCollection, NULL, obj) makes a
 * collection, vpi_create(vpiCollection, collection, obj) adds obj to it and
 * returns it (obj may be NULL), vpi_iterate(vpiMember, collection) scans
 * it and vpi_release_handle() frees it. An llg collection holds module,
 * interface and program instances and assertions, the handles 39.5.1
 * selects with (decision A17-D6). */
#define vpiCollection 810
vpiHandle vpi_create(PLI_INT32 prop, vpiHandle h, vpiHandle obj);

#ifdef __cplusplus
}
#endif

#endif /* SV_VPI_USER_H */
