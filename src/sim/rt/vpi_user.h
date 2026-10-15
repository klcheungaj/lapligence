/*
 * vpi_user.h — the VPI surface exported by a generated Lapligence model.
 *
 * This is a source-compatible subset of the IEEE 1800-2009 Annex K/M VPI
 * declarations, with the standard constant values.  Unsupported objects and properties fail through
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
typedef int64_t PLI_INT64;
typedef uint64_t PLI_UINT64;
typedef char PLI_BYTE8;

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
#define vpiTimeUnit 11
#define vpiTimePrecision 12
#define vpiScalar 17
#define vpiVector 18
#define vpiDirection 20
#define vpiInput 1
#define vpiOutput 2
#define vpiInout 3
#define vpiMixedIO 4
#define vpiNoDirection 5
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
#define vpiConstType 40
#define vpiDecConst 1
#define vpiRealConst 2
#define vpiBinaryConst 3
#define vpiOctConst 4
#define vpiHexConst 5
#define vpiStringConst 6
#define vpiIntConst 7
#define vpiTimeConst 8
#define vpiScheduled 46
#define vpiAutomatic 50
#define vpiIteratorType 57
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

vpiHandle vpi_register_cb(p_cb_data cb_data_p);
PLI_INT32 vpi_remove_cb(vpiHandle cb_obj);
void vpi_get_cb_info(vpiHandle object, p_cb_data cb_data_p);
vpiHandle vpi_register_systf(p_vpi_systf_data systf_data_p);
void vpi_get_systf_info(vpiHandle object, p_vpi_systf_data systf_data_p);

vpiHandle vpi_handle_by_name(PLI_BYTE8* name, vpiHandle scope);
vpiHandle vpi_handle_by_index(vpiHandle object, PLI_INT32 index);
vpiHandle vpi_handle_by_multi_index(vpiHandle object, PLI_INT32 num_index,
                                    PLI_INT32* index_array);
vpiHandle vpi_handle(PLI_INT32 type, vpiHandle ref_handle);
vpiHandle vpi_iterate(PLI_INT32 type, vpiHandle ref_handle);
vpiHandle vpi_scan(vpiHandle iterator);

PLI_INT32 vpi_get(PLI_INT32 property, vpiHandle object);
PLI_INT64 vpi_get64(PLI_INT32 property, vpiHandle object);
PLI_BYTE8* vpi_get_str(PLI_INT32 property, vpiHandle object);
void vpi_get_value(vpiHandle object, p_vpi_value value_p);
vpiHandle vpi_put_value(vpiHandle object, p_vpi_value value_p,
                        p_vpi_time time_p, PLI_INT32 flags);
void vpi_get_time(vpiHandle object, p_vpi_time time_p);

PLI_INT32 vpi_compare_objects(vpiHandle object1, vpiHandle object2);
PLI_INT32 vpi_chk_error(p_vpi_error_info error_info_p);
PLI_INT32 vpi_free_object(vpiHandle object);
PLI_INT32 vpi_release_handle(vpiHandle object);
PLI_INT32 vpi_get_vlog_info(p_vpi_vlog_info vlog_info_p);
void* vpi_get_userdata(vpiHandle object);
PLI_INT32 vpi_put_userdata(vpiHandle object, void* userdata);
PLI_INT32 vpi_printf(PLI_BYTE8* format, ...);
PLI_INT32 vpi_vprintf(PLI_BYTE8* format, va_list ap);
PLI_INT32 vpi_flush(void);
PLI_INT32 vpi_control(PLI_INT32 operation, ...);

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
