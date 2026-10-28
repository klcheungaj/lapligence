/*
 * vpi_user.h — the VPI surface exported by a generated Lapligence model.
 *
 * It declares every routine of the IEEE 1800-2009 Annex K listing and a
 * source-compatible subset of its types and constants, with the standard
 * values.  Unsupported routines, objects and properties fail through
 * vpi_chk_error(); they do not return guessed values.  The header is emitted
 * beside every generated model so an application can build against the same
 * ABI without depending on the vendored frontend headers.
 */
#ifndef LLG_VPI_USER_H
#define LLG_VPI_USER_H

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t PLI_INT32;
typedef uint32_t PLI_UINT32;
typedef int16_t PLI_INT16;
typedef uint16_t PLI_UINT16;
typedef int64_t PLI_INT64;
typedef uint64_t PLI_UINT64;
typedef char PLI_BYTE8;
typedef unsigned char PLI_UBYTE8;

typedef struct llg_vpi_handle* vpiHandle;

/* Object types (Annex K vpi_user.h and Annex M sv_vpi_user.h values). */
#define vpiConstant 7
#define vpiIntegerVar 25
#define vpiIterator 27
#define vpiMemoryWord 30
#define vpiModule 32
#define vpiNamedBegin 33
#define vpiNamedFork 35
#define vpiNet 36
#define vpiNetBit 37
#define vpiOperation 39
#define vpiParameter 41
#define vpiPartSelect 42
#define vpiPort 44
#define vpiRealVar 47
#define vpiReg 48
#define vpiRegBit 49
#define vpiSchedEvent 53
#define vpiSysFuncCall 56
#define vpiSysTaskCall 57
#define vpiTimeVar 63
#define vpiUserSystf 67
#define vpiCallback 107
#define vpiNetArray 114
#define vpiRange 115
#define vpiRegArray 116
#define vpiGenScope 134
#define vpiInterface 601
#define vpiLongIntVar 610
#define vpiShortIntVar 611
#define vpiIntVar 612
#define vpiShortRealVar 613
#define vpiByteVar 614
#define vpiEnumVar 617
#define vpiStructVar 618
#define vpiUnionVar 619
#define vpiBitVar 620
#define vpiPackedArrayVar 623

/* Structure objects (37.5-37.14, 37.25, 37.31, 37.37, 37.43, 37.58). */
#define vpiAlways 1
#define vpiContAssign 8
#define vpiFunction 20
#define vpiGate 21
#define vpiInitial 24
#define vpiIODecl 28
#define vpiNamedEvent 34
#define vpiPortBit 45
#define vpiPrimTerm 46
#define vpiSwitch 55
#define vpiTask 59
#define vpiUdp 65
#define vpiUdpDefn 66
#define vpiGateArray 111
#define vpiModuleArray 112
#define vpiPrimitiveArray 113
#define vpiSwitchArray 117
#define vpiUdpArray 118
#define vpiInstanceArray 121
#define vpiTaskFunc 127
#define vpiNamedEventArray 129

/* Module paths and timing checks (37.35, 37.36). */
#define vpiInterModPath 26
#define vpiModPath 31
#define vpiPathTerm 43
#define vpiTchk 61
#define vpiTchkTerm 62
#define vpiCondition 71
#define vpiTchkDataTerm 86
#define vpiTchkNotifier 87
#define vpiTchkRefTerm 88
#define vpiModDataPathIn 94
#define vpiModPathIn 95
#define vpiModPathOut 96
#define vpiPolarity 34
#define vpiDataPolarity 35
#define vpiPositive 1
#define vpiNegative 2
#define vpiUnknown 3
#define vpiEdge 36
#define vpiNoEdge 0x00
#define vpiEdge01 0x01
#define vpiEdge10 0x02
#define vpiEdge0x 0x04
#define vpiEdgex1 0x08
#define vpiEdge1x 0x10
#define vpiEdgex0 0x20
#define vpiPosedge (vpiEdgex1 | vpiEdge01 | vpiEdge0x)
#define vpiNegedge (vpiEdgex0 | vpiEdge10 | vpiEdge1x)
#define vpiAnyEdge (vpiPosedge | vpiNegedge)
#define vpiPathType 37
#define vpiPathFull 1
#define vpiPathParallel 2
#define vpiTchkType 38
#define vpiSetup 1
#define vpiHold 2
#define vpiPeriod 3
#define vpiWidth 4
#define vpiSkew 5
#define vpiRecovery 6
#define vpiNoChange 7
#define vpiSetupHold 8
#define vpiFullskew 9
#define vpiRecrem 10
#define vpiRemoval 11
#define vpiTimeskew 12
#define vpiModPathHasIfNone 71

/* Statements reported by cbForce/cbRelease/cbAssign/cbDeassign (38.36.1). */
#define vpiAssignStmt 2
#define vpiDeassign 9
#define vpiForce 16
#define vpiRelease 50

/* Statement and expression objects (37.54-37.74). */
#define vpiAssignment 3
#define vpiBegin 4
#define vpiCase 5
#define vpiCaseItem 6
#define vpiDelayControl 11
#define vpiDisable 12
#define vpiEventControl 13
#define vpiEventStmt 14
#define vpiFor 15
#define vpiForever 17
#define vpiFork 18
#define vpiFuncCall 19
#define vpiIf 22
#define vpiIfElse 23
#define vpiNullStmt 38
#define vpiRepeat 51
#define vpiRepeatControl 52
#define vpiTaskCall 60
#define vpiVarSelect 68
#define vpiWait 69
#define vpiWhile 70
#define vpiBitSelect 106
#define vpiIndexedPartSelect 130
#define vpiElseStmt 73
#define vpiForIncStmt 74
#define vpiForInitStmt 75
#define vpiOperand 97
#define vpiBaseExpr 131
#define vpiWidthExpr 132
#define vpiOpType 39
#define vpiMinusOp 1
#define vpiPlusOp 2
#define vpiNotOp 3
#define vpiBitNegOp 4
#define vpiUnaryAndOp 5
#define vpiUnaryNandOp 6
#define vpiUnaryOrOp 7
#define vpiUnaryNorOp 8
#define vpiUnaryXorOp 9
#define vpiUnaryXNorOp 10
#define vpiSubOp 11
#define vpiDivOp 12
#define vpiModOp 13
#define vpiEqOp 14
#define vpiNeqOp 15
#define vpiCaseEqOp 16
#define vpiCaseNeqOp 17
#define vpiGtOp 18
#define vpiGeOp 19
#define vpiLtOp 20
#define vpiLeOp 21
#define vpiLShiftOp 22
#define vpiRShiftOp 23
#define vpiAddOp 24
#define vpiMultOp 25
#define vpiLogAndOp 26
#define vpiLogOrOp 27
#define vpiBitAndOp 28
#define vpiBitOrOp 29
#define vpiBitXorOp 30
#define vpiBitXNorOp 31
#define vpiBitXnorOp vpiBitXNorOp
#define vpiConditionOp 32
#define vpiConcatOp 33
#define vpiMultiConcatOp 34
#define vpiEventOrOp 35
#define vpiNullOp 36
#define vpiListOp 37
#define vpiMinTypMaxOp 38
#define vpiPosedgeOp 39
#define vpiNegedgeOp 40
#define vpiArithLShiftOp 41
#define vpiArithRShiftOp 42
#define vpiPowerOp 43
#define vpiBlocking 41
#define vpiCaseType 42
#define vpiCaseExact 1
#define vpiCaseX 2
#define vpiCaseZ 3
#define vpiConstantSelect 53
#define vpiDecompile 54
#define vpiIndexedPartSelectType 72
#define vpiPosIndexed 1
#define vpiNegIndexed 2

/* Scalar logic values. */
#define vpi0 0
#define vpi1 1
#define vpiZ 2
#define vpiX 3
#define vpiH 4
#define vpiL 5
#define vpiDontCare 6

/* Strength values. */
#define vpiSupplyDrive 0x80
#define vpiStrongDrive 0x40
#define vpiPullDrive 0x20
#define vpiWeakDrive 0x08
#define vpiHiZ 0x01

/* One-to-one and one-to-many traversal methods. */
#define vpiIndex 78
#define vpiLeftRange 79
#define vpiLowConn 80
#define vpiParent 81
#define vpiRightRange 83
#define vpiScope 84
#define vpiSysTfCall 85
#define vpiArgument 89
#define vpiBit 90
#define vpiInternalScope 92
#define vpiVariables 100
#define vpiDelay 72
#define vpiHighConn 76
#define vpiLhs 77
#define vpiRhs 82
#define vpiProcess 99
#define vpiExpr 102
#define vpiPrimitive 103
#define vpiStmt 104
#define vpiPorts 125

/* Generic properties and the module/net/value properties. */
#define vpiUndefined (-1)
#define vpiType 1
#define vpiName 2
#define vpiFullName 3
#define vpiSize 4
#define vpiFile 5
#define vpiLineNo 6
#define vpiTopModule 7
#define vpiDefName 9
#define vpiProtected 10
#define vpiTimeUnit 11
#define vpiTimePrecision 12
#define vpiScalar 17
#define vpiVector 18
#define vpiExplicitName 19
#define vpiDirection 20
#define vpiInput 1
#define vpiOutput 2
#define vpiInout 3
#define vpiMixedIO 4
#define vpiNoDirection 5
#define vpiConnByName 21
#define vpiNetType 22
#define vpiWire 1
#define vpiWand 2
#define vpiWor 3
#define vpiTri 4
#define vpiTri0 5
#define vpiTri1 6
#define vpiTriReg 7
#define vpiTriAnd 8
#define vpiTriOr 9
#define vpiSupply1 10
#define vpiSupply0 11
#define vpiNone 12
#define vpiUwire 13
#define vpiArray 28
#define vpiPortIndex 29
#define vpiTermIndex 30
#define vpiStrength0 31
#define vpiStrength1 32
#define vpiPrimType 33
#define vpiAndPrim 1
#define vpiNandPrim 2
#define vpiNorPrim 3
#define vpiOrPrim 4
#define vpiXorPrim 5
#define vpiXnorPrim 6
#define vpiBufPrim 7
#define vpiNotPrim 8
#define vpiBufif0Prim 9
#define vpiBufif1Prim 10
#define vpiNotif0Prim 11
#define vpiNotif1Prim 12
#define vpiNmosPrim 13
#define vpiPmosPrim 14
#define vpiCmosPrim 15
#define vpiRnmosPrim 16
#define vpiRpmosPrim 17
#define vpiRcmosPrim 18
#define vpiRtranPrim 19
#define vpiRtranif0Prim 20
#define vpiRtranif1Prim 21
#define vpiTranPrim 22
#define vpiTranif0Prim 23
#define vpiTranif1Prim 24
#define vpiPullupPrim 25
#define vpiPulldownPrim 26
#define vpiSeqPrim 27
#define vpiCombPrim 28
#define vpiConstType 40
#define vpiDecConst 1
#define vpiRealConst 2
#define vpiBinaryConst 3
#define vpiOctConst 4
#define vpiHexConst 5
#define vpiStringConst 6
#define vpiIntConst 7
#define vpiTimeConst 8
#define vpiNetDeclAssign 43
#define vpiScheduled 46
#define vpiAutomatic 50
#define vpiIteratorType 57
#define vpiOffset 60
#define vpiValid 64
#define vpiValidFalse 0
#define vpiValidTrue 1
#define vpiSigned 65
#define vpiLocalParam 70
#define vpiIsMemory 73
#define vpiArrayType 606
#define vpiStaticArray 1
#define vpiArrayMember 607

/* System-task/function registration. */
#define vpiSysTask 1
#define vpiSysFunc 2
#define vpiIntFunc 1
#define vpiRealFunc 2
#define vpiTimeFunc 3
#define vpiSizedFunc 4
#define vpiSizedSignedFunc 5
#define vpiFuncType 44
#define vpiSysFuncType vpiFuncType
#define vpiUserDefn 45

typedef struct t_vpi_systf_data {
    PLI_INT32 type;
    PLI_INT32 sysfunctype;
    PLI_BYTE8* tfname;
    PLI_INT32 (*calltf)(PLI_BYTE8*);
    PLI_INT32 (*compiletf)(PLI_BYTE8*);
    PLI_INT32 (*sizetf)(PLI_BYTE8*);
    PLI_BYTE8* user_data;
} s_vpi_systf_data, *p_vpi_systf_data;

/* Time/value structures follow IEEE 1364/1800 layout. */
#ifndef VPI_TIME
#define VPI_TIME
typedef struct t_vpi_time {
    PLI_INT32 type;
    PLI_UINT32 high;
    PLI_UINT32 low;
    double real;
} s_vpi_time, *p_vpi_time;

#define vpiScaledRealTime 1
#define vpiSimTime 2
#define vpiSuppressTime 3
#endif

/* Delay records of vpi_get_delays()/vpi_put_delays() (38.10, 38.32). */
typedef struct t_vpi_delay {
    struct t_vpi_time* da;
    PLI_INT32 no_of_delays;
    PLI_INT32 time_type;
    PLI_INT32 mtm_flag;
    PLI_INT32 append_flag;
    PLI_INT32 pulsere_flag;
} s_vpi_delay, *p_vpi_delay;

#ifndef VPI_VECVAL
#define VPI_VECVAL
typedef struct t_vpi_vecval {
    PLI_UINT32 aval;
    PLI_UINT32 bval;
} s_vpi_vecval, *p_vpi_vecval;
#endif

typedef struct t_vpi_strengthval {
    PLI_INT32 logic;
    PLI_INT32 s0;
    PLI_INT32 s1;
} s_vpi_strengthval, *p_vpi_strengthval;

typedef struct t_vpi_value {
    PLI_INT32 format;
    union {
        PLI_BYTE8* str;
        PLI_INT32 scalar;
        PLI_INT32 integer;
        double real;
        s_vpi_time* time;
        s_vpi_vecval* vector;
        s_vpi_strengthval* strength;
        PLI_BYTE8* misc;
    } value;
} s_vpi_value, *p_vpi_value;

#define vpiBinStrVal 1
#define vpiOctStrVal 2
#define vpiDecStrVal 3
#define vpiHexStrVal 4
#define vpiScalarVal 5
#define vpiIntVal 6
#define vpiRealVal 7
#define vpiStringVal 8
#define vpiVectorVal 9
#define vpiStrengthVal 10
#define vpiTimeVal 11
#define vpiObjTypeVal 12
#define vpiSuppressVal 13
#define vpiShortIntVal 14
#define vpiLongIntVal 15
#define vpiShortRealVal 16
#define vpiRawTwoStateVal 17
#define vpiRawFourStateVal 18

/* Array value record of vpi_get_value_array()/vpi_put_value_array() (38.16,
 * 38.35). `longints` is PLI_INT64*, as the 38.16 definition gives it; the
 * Annex K listing's PLI_INT16* is a misprint of that field. */
typedef struct t_vpi_arrayvalue {
    PLI_UINT32 format;
    PLI_UINT32 flags;
    union {
        PLI_INT32* integers;
        PLI_INT16* shortints;
        PLI_INT64* longints;
        PLI_BYTE8* rawvals;
        struct t_vpi_vecval* vectors;
        struct t_vpi_time* times;
        double* reals;
        float* shortreals;
    } value;
} s_vpi_arrayvalue, *p_vpi_arrayvalue;

#define vpiNoDelay 1
#define vpiInertialDelay 2
#define vpiTransportDelay 3
#define vpiPureTransportDelay 4
#define vpiForceFlag 5
#define vpiReleaseFlag 6
#define vpiCancelEvent 7
#define vpiReturnEvent 0x1000
#define vpiUserAllocFlag 0x2000
#define vpiOneValue 0x4000
#define vpiPropagateOff 0x8000

/* Callback records (38.36). llg runs cbValueChange, the time reasons, and
 * cbEndOfCompile/cbStartOfSimulation/cbEndOfSimulation; registering any
 * other reason is a vpi_chk_error() error. */
typedef struct t_cb_data {
    PLI_INT32 reason;
    PLI_INT32 (*cb_rtn)(struct t_cb_data*);
    vpiHandle obj;
    s_vpi_time* time;
    s_vpi_value* value;
    PLI_INT32 index;
    PLI_BYTE8* user_data;
} s_cb_data, *p_cb_data;

#define cbValueChange 1
#define cbStmt 2
#define cbForce 3
#define cbRelease 4
#define cbAtStartOfSimTime 5
#define cbReadWriteSynch 6
#define cbReadOnlySynch 7
#define cbNextSimTime 8
#define cbAfterDelay 9
#define cbEndOfCompile 10
#define cbStartOfSimulation 11
#define cbEndOfSimulation 12
#define cbError 13
#define cbTchkViolation 14
#define cbStartOfSave 15
#define cbEndOfSave 16
#define cbStartOfRestart 17
#define cbEndOfRestart 18
#define cbStartOfReset 19
#define cbEndOfReset 20
#define cbEnterInteractive 21
#define cbExitInteractive 22
#define cbInteractiveScopeChange 23
#define cbUnresolvedSystf 24
#define cbAssign 25
#define cbDeassign 26
#define cbDisable 27
#define cbPLIError 28
#define cbSignal 29
#define cbNBASynch 30
#define cbAtEndOfSimTime 31

typedef struct t_vpi_vlog_info {
    PLI_INT32 argc;
    PLI_BYTE8** argv;
    PLI_BYTE8* product;
    PLI_BYTE8* version;
} s_vpi_vlog_info, *p_vpi_vlog_info;

typedef struct t_vpi_error_info {
    PLI_INT32 state;
    PLI_INT32 level;
    PLI_BYTE8* message;
    PLI_BYTE8* product;
    PLI_BYTE8* code;
    PLI_BYTE8* file;
    PLI_INT32 line;
} s_vpi_error_info, *p_vpi_error_info;

#define vpiCompile 1
#define vpiPLI 2
#define vpiRun 3
#define vpiNotice 1
#define vpiWarning 2
#define vpiError 3
#define vpiSystem 4
#define vpiInternal 5

/* Control operations. */
#define vpiStop 66
#define vpiFinish 67
#define vpiReset 68
#define vpiSetInteractiveScope 69

/* The 41 routines of Annex K, in its order. Those llg does not provide yet
 * (vpi_handle_multi, vpi_get_delays/vpi_put_delays,
 * vpi_get_value_array/vpi_put_value_array, vpi_get_data/vpi_put_data) are
 * defined so that applications link, and fail through vpi_chk_error()
 * (LLG_VPI_UNSUPPORTED) without touching their arguments. */
vpiHandle vpi_register_cb(p_cb_data cb_data_p);
PLI_INT32 vpi_remove_cb(vpiHandle cb_obj);
void vpi_get_cb_info(vpiHandle object, p_cb_data cb_data_p);
vpiHandle vpi_register_systf(p_vpi_systf_data systf_data_p);
void vpi_get_systf_info(vpiHandle object, p_vpi_systf_data systf_data_p);

vpiHandle vpi_handle_by_name(PLI_BYTE8* name, vpiHandle scope);
vpiHandle vpi_handle_by_index(vpiHandle object, PLI_INT32 index);

vpiHandle vpi_handle(PLI_INT32 type, vpiHandle ref_handle);
vpiHandle vpi_handle_multi(PLI_INT32 type, vpiHandle ref_handle1,
                           vpiHandle ref_handle2, ...);
vpiHandle vpi_iterate(PLI_INT32 type, vpiHandle ref_handle);
vpiHandle vpi_scan(vpiHandle iterator);

PLI_INT32 vpi_get(PLI_INT32 property, vpiHandle object);
PLI_INT64 vpi_get64(PLI_INT32 property, vpiHandle object);
PLI_BYTE8* vpi_get_str(PLI_INT32 property, vpiHandle object);

void vpi_get_delays(vpiHandle object, p_vpi_delay delay_p);
void vpi_put_delays(vpiHandle object, p_vpi_delay delay_p);

void vpi_get_value(vpiHandle object, p_vpi_value value_p);
vpiHandle vpi_put_value(vpiHandle object, p_vpi_value value_p,
                        p_vpi_time time_p, PLI_INT32 flags);
void vpi_get_value_array(vpiHandle object, p_vpi_arrayvalue arrayvalue_p,
                         PLI_INT32* index_p, PLI_UINT32 num);
void vpi_put_value_array(vpiHandle object, p_vpi_arrayvalue arrayvalue_p,
                         PLI_INT32* index_p, PLI_UINT32 num);

void vpi_get_time(vpiHandle object, p_vpi_time time_p);

/* Multichannel descriptors are shared with $fopen/$fdisplay/$fclose: bit 0
 * is the standard output, bits 1-30 are files, and a descriptor with bit 31
 * set is a $fopen file descriptor, which these routines refuse except for
 * vpi_mcd_name(). */
PLI_UINT32 vpi_mcd_open(PLI_BYTE8* fileName);
PLI_UINT32 vpi_mcd_close(PLI_UINT32 mcd);
PLI_BYTE8* vpi_mcd_name(PLI_UINT32 cd);
PLI_INT32 vpi_mcd_printf(PLI_UINT32 mcd, PLI_BYTE8* format, ...);
PLI_INT32 vpi_printf(PLI_BYTE8* format, ...);

PLI_INT32 vpi_compare_objects(vpiHandle object1, vpiHandle object2);
PLI_INT32 vpi_chk_error(p_vpi_error_info error_info_p);
PLI_INT32 vpi_free_object(vpiHandle object);
PLI_INT32 vpi_release_handle(vpiHandle object);
PLI_INT32 vpi_get_vlog_info(p_vpi_vlog_info vlog_info_p);

PLI_INT32 vpi_get_data(PLI_INT32 id, PLI_BYTE8* dataLoc, PLI_INT32 numOfBytes);
PLI_INT32 vpi_put_data(PLI_INT32 id, PLI_BYTE8* dataLoc, PLI_INT32 numOfBytes);
void* vpi_get_userdata(vpiHandle object);
PLI_INT32 vpi_put_userdata(vpiHandle object, void* userdata);
PLI_INT32 vpi_vprintf(PLI_BYTE8* format, va_list ap);
PLI_INT32 vpi_mcd_vprintf(PLI_UINT32 mcd, PLI_BYTE8* format, va_list ap);
PLI_INT32 vpi_flush(void);
PLI_INT32 vpi_mcd_flush(PLI_UINT32 mcd);
PLI_INT32 vpi_control(PLI_INT32 operation, ...);
vpiHandle vpi_handle_by_multi_index(vpiHandle object, PLI_INT32 num_index,
                                    PLI_INT32* index_array);

/* Startup entry point expected from a VPI shared object. As in the Annex K
 * header, a Windows DLL exports it (PLI_DLLESPEC), so the model finds it with
 * GetProcAddress; elsewhere the attribute is empty. */
#ifndef PLI_DLLESPEC
#if defined(_MSC_VER) || defined(__MINGW32__) || defined(__CYGWIN__)
#define PLI_DLLESPEC __declspec(dllexport)
#else
#define PLI_DLLESPEC
#endif
#endif
extern PLI_DLLESPEC void (*vlog_startup_routines[])(void);

#ifdef __cplusplus
}
#endif

#endif /* LLG_VPI_USER_H */
