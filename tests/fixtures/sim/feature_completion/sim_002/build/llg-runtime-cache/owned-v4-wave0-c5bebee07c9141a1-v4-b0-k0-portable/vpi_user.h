/*
 * vpi_user.h — the bounded VPI surface exported by a generated Lapligence
 * model.
 *
 * This is intentionally a small, source-compatible subset of the IEEE VPI
 * declarations.  Unsupported objects and properties fail through
 * vpi_chk_error(); they do not return guessed values.  The header is emitted
 * beside every generated model so an application can build against the same
 * ABI without depending on the vendored frontend headers.
 */
#ifndef LLG_VPI_USER_H
#define LLG_VPI_USER_H

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

/* Object types used by the H28 vertical slice. */
#define vpiModule 32
#define vpiNet 36
#define vpiRealVar 47
#define vpiReg 48
#define vpiUserSystf 67
#define vpiCallback 107
#define vpiIterator 27
#define vpiRegArray 116
#define vpiNetArray 114
#define vpiIteratorType 57
#define vpiSysFuncCall 56
#define vpiSysTaskCall 57
#define vpiSysTfCall 85
#define vpi0 0
#define vpi1 1
#define vpiZ 2
#define vpiX 3

/* One-to-one and one-to-many traversal methods. */
#define vpiParent 81
#define vpiScope 84
#define vpiArgument 89
#define vpiVariables 100
#define vpiInternalScope 92
#define vpiPorts 125

/* Generic properties and the bounded module/net/value properties. */
#define vpiUndefined (-1)
#define vpiType 1
#define vpiName 2
#define vpiFullName 3
#define vpiSize 4
#define vpiFile 5
#define vpiLineNo 6
#define vpiDefName 9
#define vpiTopModule 7
#define vpiScalar 17
#define vpiVector 18
#define vpiDirection 20
#define vpiNetType 22
#define vpiWire 1
#define vpiSigned 65
#define vpiValid 64
#define vpiValidFalse 0
#define vpiValidTrue 1

/* System-task/function registration. */
#define vpiSysTask 1
#define vpiSysFunc 2
#define vpiIntFunc 1
#define vpiRealFunc 2
#define vpiTimeFunc 3
#define vpiSizedFunc 4
#define vpiSizedSignedFunc 5
#define vpiSysFuncType 44
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
#define vpiForceFlag 5
#define vpiReleaseFlag 6
#define vpiReturnEvent 0x1000

/* Callback records and the supported start/end reasons. */
typedef struct t_cb_data {
    PLI_INT32 reason;
    PLI_INT32 (*cb_rtn)(struct t_cb_data*);
    vpiHandle obj;
    s_vpi_time* time;
    s_vpi_value* value;
    PLI_INT32 index;
    PLI_BYTE8* user_data;
} s_cb_data, *p_cb_data;

#define cbStartOfSimulation 11
#define cbEndOfSimulation 12

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

vpiHandle vpi_register_cb(p_cb_data cb_data_p);
PLI_INT32 vpi_remove_cb(vpiHandle cb_obj);
void vpi_get_cb_info(vpiHandle object, p_cb_data cb_data_p);
vpiHandle vpi_register_systf(p_vpi_systf_data systf_data_p);
void vpi_get_systf_info(vpiHandle object, p_vpi_systf_data systf_data_p);

vpiHandle vpi_handle_by_name(PLI_BYTE8* name, vpiHandle scope);
vpiHandle vpi_handle_by_index(vpiHandle object, PLI_INT32 index);
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
PLI_INT32 vpi_get_vlog_info(void* info);
void* vpi_get_userdata(vpiHandle object);
PLI_INT32 vpi_put_userdata(vpiHandle object, void* userdata);
PLI_INT32 vpi_printf(PLI_BYTE8* format, ...);
PLI_INT32 vpi_flush(void);
PLI_INT32 vpi_control(PLI_INT32 operation, ...);

/* Startup entry point expected from a VPI shared object. */
extern void (*vlog_startup_routines[])(void);

#ifdef __cplusplus
}
#endif

#endif /* LLG_VPI_USER_H */
