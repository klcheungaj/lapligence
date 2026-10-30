/*
 * sv_vpi_user.h — the SystemVerilog VPI extensions of IEEE 1800-2009
 * Annex M: every constant, structure and routine with the standard values,
 * plus the llg additions marked below. It includes vpi_user.h, which
 * already carries the Annex M object types the runtime serves (vpiInterface,
 * the SystemVerilog variable types). Which objects, relations and
 * properties a generated model serves is recorded in vpi_catalog.txt.
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

/* Coverage type properties. Statement and toggle coverage are not
 * implemented: they fail through vpi_chk_error(). */
#define vpiAssertCoverage 760
#define vpiFsmStateCoverage 761
#define vpiStatementCoverage 762
#define vpiToggleCoverage 763

/* FSM objects (40.5.2): an FSM and one of its states. */
#define vpiFsm 758
#define vpiFsmHandle 759

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

/* FSM relations (40.5.2). */
#define vpiFsmStates 775
#define vpiFsmStateExpression 776

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

/* The remaining Annex M constants, grouped and ordered as in Annex M.
 * They are defined for source compatibility; whether llg serves an object,
 * relation or property is recorded in vpi_catalog.txt. */
/* Object types. */
#define vpiPackage 600
#define vpiTypeParameter 609
#define vpiSequenceTypespec 696
#define vpiPropertyTypespec 697
#define vpiEventTypespec 698
#define vpiClockingBlock 650
#define vpiClockingIODecl 651
#define vpiConstraint 653
#define vpiConstraintOrdering 654
#define vpiDistItem 645
#define vpiAliasStmt 646
#define vpiThread 647
#define vpiRestrict 901
#define vpiMulticlockSequenceExpr 658
#define vpiClockedSeq 659
#define vpiClockedProp 902
#define vpiCaseProperty 662
#define vpiCasePropertyItem 905
#define vpiAnyPattern 667
#define vpiTaggedPattern 668
#define vpiStructPattern 669
#define vpiExpectStmt 674
#define vpiDistribution 678
#define vpiArrayNet vpiNetArray
#define vpiEnumNet 680
#define vpiIntegerNet 681
#define vpiLogicNet vpiNet
#define vpiTimeNet 682
#define vpiStructNet 683
#define vpiPackedArrayNet 693
#define vpiConstraintExpr 747
#define vpiElseConst 748
#define vpiImplication 749
#define vpiConstrIf 738
#define vpiConstrIfElse 739
#define vpiLetDecl 903
#define vpiLetExpr 904
/* Methods used to traverse 1 to 1 relationships. */
#define vpiInputSkew 706
#define vpiOutputSkew 707
#define vpiGlobalClocking 708
#define vpiDefaultClocking 709
#define vpiDefaultDisableIff 710
#define vpiOrigin 713
#define vpiPrefix 714
#define vpiWith 715
#define vpiValueRange 720
#define vpiPattern 721
#define vpiWeight 722
#define vpiConstraintItem 746
/* Methods used to traverse 1 to many relationships. */
#define vpiImport 726
#define vpiInterfaceDecl 728
#define vpiSolveBefore 731
#define vpiSolveAfter 732
#define vpiMessages 735
#define vpiConcurrentAssertions 740
#define vpiMatchItem 741
/* Generic object properties. */
#define vpiTop 600
#define vpiUnit 602
#define vpiAccessType 604
#define vpiForkJoinAcc 1
#define vpiExternAcc 2
#define vpiDPIExportAcc 3
#define vpiDPIImportAcc 4
#define vpiIsRandomized 608
#define vpiOpStrong 656
#define vpiRandType 610
#define vpiNotRand 1
#define vpiRand 2
#define vpiRandC 3
#define vpiConstantVariable 612
#define vpiProtectedVis 2
#define vpiLocalVis 3
#define vpiOneStepConst 9
#define vpiDistType 625
#define vpiEqualDist 1
#define vpiDivDist 2
#define vpiRef 6
#define vpiVirtual 635
#define vpiIsConstraintEnabled 638
#define vpiTaggedQualifier 4
#define vpiRandQualifier 8
#define vpiInputEdge 651
#define vpiOutputEdge 652
#define vpiGeneric 653
#define vpiCompatibilityMode 654
#define vpiMode1364v1995 1
#define vpiMode1364v2001 2
#define vpiMode1364v2005 3
#define vpiMode1800v2005 4
#define vpiMode1800v2009 5
#define vpiPackedArrayMember 655
#define vpiDPIPure 665
#define vpiDPIContext 666
#define vpiDPICStr 667
#define vpiDPI 1
#define vpiDPIC 2
#define vpiDPICIdentifier 668
/* Operators. */
#define vpiNonOverlapImplyOp 51
#define vpiOverlapImplyOp 52
#define vpiAcceptOnOp 83
#define vpiRejectOnOp 84
#define vpiSyncAcceptOnOp 85
#define vpiSyncRejectOnOp 86
#define vpiOverlapFollowedByOp 87
#define vpiNonOverlapFollowedByOp 88
#define vpiNexttimeOp 89
#define vpiAlwaysOp 90
#define vpiEventuallyOp 91
#define vpiUntilOp 92
#define vpiUntilWithOp 93
#define vpiUnaryCycleDelayOp 53
#define vpiCycleDelayOp 54
#define vpiIntersectOp 55
#define vpiFirstMatchOp 56
#define vpiThroughoutOp 57
#define vpiWithinOp 58
#define vpiRepeatOp 59
#define vpiConsecutiveRepeatOp 60
#define vpiGotoRepeatOp 61
#define vpiMatchOp 66
#define vpiMatchedOp 73
#define vpiTriggeredOp 74
#define vpiIfOp 77
#define vpiIfElseOp 78
#define vpiCompAndOp 79
#define vpiCompOrOp 80
#define vpiImpliesOp 94
#define vpiTypeOp 81
/* Value for vpiValid. */
#define vpiValidUnknown 2
/* Callback reasons. */
#define cbStartOfThread 600
#define cbEndOfThread 601
#define cbEnterThread 602
#define cbStartOfFrame 603
#define cbEndOfFrame 604
/* FSM relations and object types (Annex M, 40.5) */
#define vpiFsmStates 775
#define vpiFsmStateExpression 776
#define vpiFsm 758
#define vpiFsmHandle 759

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
