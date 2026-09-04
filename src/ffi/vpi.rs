//! Rust bindings for the VPI / UHDM C API.
//!
//! Covers:
//!  - `vpi_user.h`      — IEEE 1800-2017 VPI constants, types, and functions
//!  - `uhdm_vpi_user.h` — UHDM-specific extension constants
//!  - `uhdm_types.h`    — `UHDM_OBJECT_TYPE` enum values (as `pub const`)
//!
//! # Usage from a binary crate
//! Add `#[path = "../vpi_user.rs"] mod vpi_user;` (or `pub mod`) at the top
//! of any `src/bin/*.rs` file, or move to a shared lib crate.
//!
//! All VPI functions are in the `uhdm` static library (already linked via
//! `build.rs`); no extra `#[link]` attribute is needed.

// VPI handles are opaque C tokens: these wrappers pass them back to UHDM but
// never dereference them in Rust. Multiple `#[link]` attributes are required
// to make every native archive propagate through the Rust library target.
#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::duplicated_attributes,
    clippy::not_unsafe_ptr_arg_deref
)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_float, c_int, c_short, c_uint, c_void};

// ── Primitive type aliases ────────────────────────────────────────────────────

pub type PLI_INT32 = c_int;
pub type PLI_UINT32 = c_uint;
pub type PLI_INT16 = c_short;
pub type PLI_UINT16 = u16;
pub type PLI_BYTE8 = c_char;
pub type PLI_UBYTE8 = u8;
pub type PLI_INT64 = i64;
pub type PLI_UINT64 = u64;

/// VPI handle: a pointer to an opaque `PLI_UINT32` object.
pub type VpiHandle = *mut PLI_UINT32;

// ── VPI object types (vpi_user.h §OBJECT TYPES) ───────────────────────────────

pub const vpiAlways: PLI_INT32 = 1;
pub const vpiAssignStmt: PLI_INT32 = 2;
pub const vpiAssignment: PLI_INT32 = 3;
pub const vpiBegin: PLI_INT32 = 4;
pub const vpiCase: PLI_INT32 = 5;
pub const vpiCaseItem: PLI_INT32 = 6;
pub const vpiConstant: PLI_INT32 = 7;
pub const vpiContAssign: PLI_INT32 = 8;
pub const vpiDeassign: PLI_INT32 = 9;
pub const vpiDefParam: PLI_INT32 = 10;
pub const vpiDelayControl: PLI_INT32 = 11;
pub const vpiDisable: PLI_INT32 = 12;
pub const vpiEventControl: PLI_INT32 = 13;
pub const vpiEventStmt: PLI_INT32 = 14;
pub const vpiFor: PLI_INT32 = 15;
pub const vpiForce: PLI_INT32 = 16;
pub const vpiForever: PLI_INT32 = 17;
pub const vpiFork: PLI_INT32 = 18;
pub const vpiFuncCall: PLI_INT32 = 19;
pub const vpiFunction: PLI_INT32 = 20;
pub const vpiGate: PLI_INT32 = 21;
pub const vpiIf: PLI_INT32 = 22;
pub const vpiIfElse: PLI_INT32 = 23;
pub const vpiInitial: PLI_INT32 = 24;
pub const vpiIntegerVar: PLI_INT32 = 25;
pub const vpiInterModPath: PLI_INT32 = 26;
pub const vpiIterator: PLI_INT32 = 27;
pub const vpiIODecl: PLI_INT32 = 28;
pub const vpiMemory: PLI_INT32 = 29;
pub const vpiMemoryWord: PLI_INT32 = 30;
pub const vpiModPath: PLI_INT32 = 31;
pub const vpiModule: PLI_INT32 = 32;
pub const vpiNamedBegin: PLI_INT32 = 33;
pub const vpiNamedEvent: PLI_INT32 = 34;
pub const vpiNamedFork: PLI_INT32 = 35;
pub const vpiNet: PLI_INT32 = 36;
pub const vpiNetBit: PLI_INT32 = 37;
pub const vpiNullStmt: PLI_INT32 = 38;
pub const vpiOperation: PLI_INT32 = 39;
pub const vpiParamAssign: PLI_INT32 = 40;
pub const vpiParameter: PLI_INT32 = 41;
pub const vpiPartSelect: PLI_INT32 = 42;
pub const vpiPathTerm: PLI_INT32 = 43;
pub const vpiPort: PLI_INT32 = 44;
pub const vpiPortBit: PLI_INT32 = 45;
pub const vpiPrimTerm: PLI_INT32 = 46;
pub const vpiRealVar: PLI_INT32 = 47;
pub const vpiReg: PLI_INT32 = 48;
pub const vpiRegBit: PLI_INT32 = 49;
pub const vpiRelease: PLI_INT32 = 50;
pub const vpiRepeat: PLI_INT32 = 51;
pub const vpiRepeatControl: PLI_INT32 = 52;
pub const vpiSchedEvent: PLI_INT32 = 53;
pub const vpiSpecParam: PLI_INT32 = 54;
pub const vpiSwitch: PLI_INT32 = 55;
pub const vpiSysFuncCall: PLI_INT32 = 56;
pub const vpiSysTaskCall: PLI_INT32 = 57;
pub const vpiTableEntry: PLI_INT32 = 58;
pub const vpiTask: PLI_INT32 = 59;
pub const vpiTaskCall: PLI_INT32 = 60;
pub const vpiTchk: PLI_INT32 = 61;
pub const vpiTchkTerm: PLI_INT32 = 62;
pub const vpiTimeVar: PLI_INT32 = 63;
pub const vpiTimeQueue: PLI_INT32 = 64;
pub const vpiUdp: PLI_INT32 = 65;
pub const vpiUdpDefn: PLI_INT32 = 66;
pub const vpiUserSystf: PLI_INT32 = 67;
pub const vpiVarSelect: PLI_INT32 = 68;
pub const vpiWait: PLI_INT32 = 69;
pub const vpiWhile: PLI_INT32 = 70;

// 1364-2001 object types
pub const vpiAttribute: PLI_INT32 = 105;
pub const vpiBitSelect: PLI_INT32 = 106;
pub const vpiCallback: PLI_INT32 = 107;
pub const vpiDelayTerm: PLI_INT32 = 108;
pub const vpiDelayDevice: PLI_INT32 = 109;
pub const vpiFrame: PLI_INT32 = 110;
pub const vpiGateArray: PLI_INT32 = 111;
pub const vpiModuleArray: PLI_INT32 = 112;
pub const vpiPrimitiveArray: PLI_INT32 = 113;
pub const vpiNetArray: PLI_INT32 = 114;
pub const vpiRange: PLI_INT32 = 115;
pub const vpiRegArray: PLI_INT32 = 116;
pub const vpiSwitchArray: PLI_INT32 = 117;
pub const vpiUdpArray: PLI_INT32 = 118;
pub const vpiContAssignBit: PLI_INT32 = 128;
pub const vpiNamedEventArray: PLI_INT32 = 129;

// 1364-2005 object types
pub const vpiIndexedPartSelect: PLI_INT32 = 130;
pub const vpiGenScopeArray: PLI_INT32 = 133;
pub const vpiGenScope: PLI_INT32 = 134;
pub const vpiGenVar: PLI_INT32 = 135;

// ── VPI methods (1-to-1 relationships) ───────────────────────────────────────

pub const vpiCondition: PLI_INT32 = 71;
pub const vpiDelay: PLI_INT32 = 72;
pub const vpiElseStmt: PLI_INT32 = 73;
pub const vpiForIncStmt: PLI_INT32 = 74;
pub const vpiForInitStmt: PLI_INT32 = 75;
pub const vpiHighConn: PLI_INT32 = 76;
pub const vpiLhs: PLI_INT32 = 77;
pub const vpiIndex: PLI_INT32 = 78;
pub const vpiLeftRange: PLI_INT32 = 79;
pub const vpiLowConn: PLI_INT32 = 80;
pub const vpiParent: PLI_INT32 = 81;
pub const vpiRhs: PLI_INT32 = 82;
pub const vpiRightRange: PLI_INT32 = 83;
pub const vpiScope: PLI_INT32 = 84;
pub const vpiSysTfCall: PLI_INT32 = 85;
pub const vpiTchkDataTerm: PLI_INT32 = 86;
pub const vpiTchkNotifier: PLI_INT32 = 87;
pub const vpiTchkRefTerm: PLI_INT32 = 88;

// VPI methods (1-to-many relationships)
pub const vpiArgument: PLI_INT32 = 89;
pub const vpiBit: PLI_INT32 = 90;
pub const vpiDriver: PLI_INT32 = 91;
pub const vpiInternalScope: PLI_INT32 = 92;
pub const vpiLoad: PLI_INT32 = 93;
pub const vpiModDataPathIn: PLI_INT32 = 94;
pub const vpiModPathIn: PLI_INT32 = 95;
pub const vpiModPathOut: PLI_INT32 = 96;
pub const vpiOperand: PLI_INT32 = 97;
pub const vpiPortInst: PLI_INT32 = 98;
pub const vpiProcess: PLI_INT32 = 99;
pub const vpiVariables: PLI_INT32 = 100;
pub const vpiUse: PLI_INT32 = 101;

// VPI methods (1-to-1 or 1-to-many)
pub const vpiExpr: PLI_INT32 = 102;
pub const vpiPrimitive: PLI_INT32 = 103;
pub const vpiStmt: PLI_INT32 = 104;

// 1364-2001 methods
pub const vpiActiveTimeFormat: PLI_INT32 = 119;
pub const vpiInTerm: PLI_INT32 = 120;
pub const vpiInstanceArray: PLI_INT32 = 121;
pub const vpiLocalDriver: PLI_INT32 = 122;
pub const vpiLocalLoad: PLI_INT32 = 123;
pub const vpiOutTerm: PLI_INT32 = 124;
pub const vpiPorts: PLI_INT32 = 125;
pub const vpiSimNet: PLI_INT32 = 126;
pub const vpiTaskFunc: PLI_INT32 = 127;

// 1364-2005 methods
pub const vpiBaseExpr: PLI_INT32 = 131;
pub const vpiWidthExpr: PLI_INT32 = 132;

// 1800-2009 methods
pub const vpiAutomatics: PLI_INT32 = 136;

// ── VPI properties ────────────────────────────────────────────────────────────

pub const vpiUndefined: PLI_INT32 = -1;
pub const vpiType: PLI_INT32 = 1;
pub const vpiName: PLI_INT32 = 2;
pub const vpiFullName: PLI_INT32 = 3;
pub const vpiSize: PLI_INT32 = 4;
pub const vpiFile: PLI_INT32 = 5;
pub const vpiLineNo: PLI_INT32 = 6;

// Module properties
pub const vpiTopModule: PLI_INT32 = 7;
pub const vpiCellInstance: PLI_INT32 = 8;
pub const vpiDefName: PLI_INT32 = 9;
pub const vpiProtected: PLI_INT32 = 10;
pub const vpiTimeUnit: PLI_INT32 = 11;
pub const vpiTimePrecision: PLI_INT32 = 12;
pub const vpiDefNetType: PLI_INT32 = 13;
pub const vpiUnconnDrive: PLI_INT32 = 14;
pub const vpiHighZ: PLI_INT32 = 1; // vpiUnconnDrive subtype
pub const vpiPull1: PLI_INT32 = 2;
pub const vpiPull0: PLI_INT32 = 3;
pub const vpiDefFile: PLI_INT32 = 15;
pub const vpiDefLineNo: PLI_INT32 = 16;
pub const vpiDefDelayMode: PLI_INT32 = 47;
pub const vpiDelayModeNone: PLI_INT32 = 1; // vpiDefDelayMode subtypes
pub const vpiDelayModePath: PLI_INT32 = 2;
pub const vpiDelayModeDistrib: PLI_INT32 = 3;
pub const vpiDelayModeUnit: PLI_INT32 = 4;
pub const vpiDelayModeZero: PLI_INT32 = 5;
pub const vpiDelayModeMTM: PLI_INT32 = 6;
pub const vpiDefDecayTime: PLI_INT32 = 48;

// Port and net properties
pub const vpiScalar: PLI_INT32 = 17;
pub const vpiVector: PLI_INT32 = 18;
pub const vpiExplicitName: PLI_INT32 = 19;
pub const vpiDirection: PLI_INT32 = 20;
pub const vpiInput: PLI_INT32 = 1; // vpiDirection subtypes
pub const vpiOutput: PLI_INT32 = 2;
pub const vpiInout: PLI_INT32 = 3;
pub const vpiMixedIO: PLI_INT32 = 4;
pub const vpiNoDirection: PLI_INT32 = 5;
pub const vpiConnByName: PLI_INT32 = 21;
pub const vpiNetType: PLI_INT32 = 22;
pub const vpiWire: PLI_INT32 = 1; // vpiNetType subtypes
pub const vpiWand: PLI_INT32 = 2;
pub const vpiWor: PLI_INT32 = 3;
pub const vpiTri: PLI_INT32 = 4;
pub const vpiTri0: PLI_INT32 = 5;
pub const vpiTri1: PLI_INT32 = 6;
pub const vpiTriReg: PLI_INT32 = 7;
pub const vpiTriAnd: PLI_INT32 = 8;
pub const vpiTriOr: PLI_INT32 = 9;
pub const vpiSupply1: PLI_INT32 = 10;
pub const vpiSupply0: PLI_INT32 = 11;
pub const vpiNone: PLI_INT32 = 12;
pub const vpiUwire: PLI_INT32 = 13;
pub const vpiExplicitScalared: PLI_INT32 = 23;
pub const vpiExplicitVectored: PLI_INT32 = 24;
pub const vpiExpanded: PLI_INT32 = 25;
pub const vpiImplicitDecl: PLI_INT32 = 26;
pub const vpiChargeStrength: PLI_INT32 = 27;
pub const vpiArray: PLI_INT32 = 28;
pub const vpiPortIndex: PLI_INT32 = 29;

// Gate and terminal properties
pub const vpiTermIndex: PLI_INT32 = 30;
pub const vpiStrength0: PLI_INT32 = 31;
pub const vpiStrength1: PLI_INT32 = 32;
pub const vpiPrimType: PLI_INT32 = 33;
pub const vpiAndPrim: PLI_INT32 = 1; // vpiPrimType subtypes
pub const vpiNandPrim: PLI_INT32 = 2;
pub const vpiNorPrim: PLI_INT32 = 3;
pub const vpiOrPrim: PLI_INT32 = 4;
pub const vpiXorPrim: PLI_INT32 = 5;
pub const vpiXnorPrim: PLI_INT32 = 6;
pub const vpiBufPrim: PLI_INT32 = 7;
pub const vpiNotPrim: PLI_INT32 = 8;
pub const vpiBufif0Prim: PLI_INT32 = 9;
pub const vpiBufif1Prim: PLI_INT32 = 10;
pub const vpiNotif0Prim: PLI_INT32 = 11;
pub const vpiNotif1Prim: PLI_INT32 = 12;
pub const vpiNmosPrim: PLI_INT32 = 13;
pub const vpiPmosPrim: PLI_INT32 = 14;
pub const vpiCmosPrim: PLI_INT32 = 15;
pub const vpiRnmosPrim: PLI_INT32 = 16;
pub const vpiRpmosPrim: PLI_INT32 = 17;
pub const vpiRcmosPrim: PLI_INT32 = 18;
pub const vpiRtranPrim: PLI_INT32 = 19;
pub const vpiRtranif0Prim: PLI_INT32 = 20;
pub const vpiRtranif1Prim: PLI_INT32 = 21;
pub const vpiTranPrim: PLI_INT32 = 22;
pub const vpiTranif0Prim: PLI_INT32 = 23;
pub const vpiTranif1Prim: PLI_INT32 = 24;
pub const vpiPullupPrim: PLI_INT32 = 25;
pub const vpiPulldownPrim: PLI_INT32 = 26;
pub const vpiSeqPrim: PLI_INT32 = 27;
pub const vpiCombPrim: PLI_INT32 = 28;

// Path, timing-check properties
pub const vpiPolarity: PLI_INT32 = 34;
pub const vpiDataPolarity: PLI_INT32 = 35;
pub const vpiPositive: PLI_INT32 = 1; // polarity subtypes
pub const vpiNegative: PLI_INT32 = 2;
pub const vpiUnknown: PLI_INT32 = 3;

pub const vpiEdge: PLI_INT32 = 36;
pub const vpiNoEdge: PLI_INT32 = 0x00;
pub const vpiEdge01: PLI_INT32 = 0x01;
pub const vpiEdge10: PLI_INT32 = 0x02;
pub const vpiEdge0x: PLI_INT32 = 0x04;
pub const vpiEdgex1: PLI_INT32 = 0x08;
pub const vpiEdge1x: PLI_INT32 = 0x10;
pub const vpiEdgex0: PLI_INT32 = 0x20;
pub const vpiPosedge: PLI_INT32 = vpiEdgex1 | vpiEdge01 | vpiEdge0x; // 0x0D
pub const vpiNegedge: PLI_INT32 = vpiEdgex0 | vpiEdge10 | vpiEdge1x; // 0x32
pub const vpiAnyEdge: PLI_INT32 = vpiPosedge | vpiNegedge; // 0x3F

pub const vpiPathType: PLI_INT32 = 37;
pub const vpiPathFull: PLI_INT32 = 1; // vpiPathType subtypes
pub const vpiPathParallel: PLI_INT32 = 2;

pub const vpiTchkType: PLI_INT32 = 38;
pub const vpiSetup: PLI_INT32 = 1; // vpiTchkType subtypes
pub const vpiHold: PLI_INT32 = 2;
pub const vpiPeriod: PLI_INT32 = 3;
pub const vpiWidth: PLI_INT32 = 4;
pub const vpiSkew: PLI_INT32 = 5;
pub const vpiRecovery: PLI_INT32 = 6;
pub const vpiNoChange: PLI_INT32 = 7;
pub const vpiSetupHold: PLI_INT32 = 8;
pub const vpiFullskew: PLI_INT32 = 9;
pub const vpiRecrem: PLI_INT32 = 10;
pub const vpiRemoval: PLI_INT32 = 11;
pub const vpiTimeskew: PLI_INT32 = 12;

// Expression properties
pub const vpiOpType: PLI_INT32 = 39;
pub const vpiMinusOp: PLI_INT32 = 1; // vpiOpType subtypes
pub const vpiPlusOp: PLI_INT32 = 2;
pub const vpiNotOp: PLI_INT32 = 3;
pub const vpiBitNegOp: PLI_INT32 = 4;
pub const vpiUnaryAndOp: PLI_INT32 = 5;
pub const vpiUnaryNandOp: PLI_INT32 = 6;
pub const vpiUnaryOrOp: PLI_INT32 = 7;
pub const vpiUnaryNorOp: PLI_INT32 = 8;
pub const vpiUnaryXorOp: PLI_INT32 = 9;
pub const vpiUnaryXNorOp: PLI_INT32 = 10;
pub const vpiSubOp: PLI_INT32 = 11;
pub const vpiDivOp: PLI_INT32 = 12;
pub const vpiModOp: PLI_INT32 = 13;
pub const vpiEqOp: PLI_INT32 = 14;
pub const vpiNeqOp: PLI_INT32 = 15;
pub const vpiCaseEqOp: PLI_INT32 = 16;
pub const vpiCaseNeqOp: PLI_INT32 = 17;
pub const vpiGtOp: PLI_INT32 = 18;
pub const vpiGeOp: PLI_INT32 = 19;
pub const vpiLtOp: PLI_INT32 = 20;
pub const vpiLeOp: PLI_INT32 = 21;
pub const vpiLShiftOp: PLI_INT32 = 22;
pub const vpiRShiftOp: PLI_INT32 = 23;
pub const vpiAddOp: PLI_INT32 = 24;
pub const vpiMultOp: PLI_INT32 = 25;
pub const vpiLogAndOp: PLI_INT32 = 26;
pub const vpiLogOrOp: PLI_INT32 = 27;
pub const vpiBitAndOp: PLI_INT32 = 28;
pub const vpiBitOrOp: PLI_INT32 = 29;
pub const vpiBitXorOp: PLI_INT32 = 30;
pub const vpiBitXNorOp: PLI_INT32 = 31;
pub const vpiBitXnorOp: PLI_INT32 = vpiBitXNorOp; // 1364-2001 alias
pub const vpiConditionOp: PLI_INT32 = 32;
pub const vpiConcatOp: PLI_INT32 = 33;
pub const vpiMultiConcatOp: PLI_INT32 = 34;
pub const vpiEventOrOp: PLI_INT32 = 35;
pub const vpiNullOp: PLI_INT32 = 36;
pub const vpiListOp: PLI_INT32 = 37;
pub const vpiMinTypMaxOp: PLI_INT32 = 38;
pub const vpiPosedgeOp: PLI_INT32 = 39;
pub const vpiNegedgeOp: PLI_INT32 = 40;
pub const vpiArithLShiftOp: PLI_INT32 = 41;
pub const vpiArithRShiftOp: PLI_INT32 = 42;
pub const vpiPowerOp: PLI_INT32 = 43;

pub const vpiConstType: PLI_INT32 = 40;
pub const vpiDecConst: PLI_INT32 = 1; // vpiConstType subtypes
pub const vpiRealConst: PLI_INT32 = 2;
pub const vpiBinaryConst: PLI_INT32 = 3;
pub const vpiOctConst: PLI_INT32 = 4;
pub const vpiHexConst: PLI_INT32 = 5;
pub const vpiStringConst: PLI_INT32 = 6;
pub const vpiIntConst: PLI_INT32 = 7;
pub const vpiTimeConst: PLI_INT32 = 8;
pub const vpiUIntConst: PLI_INT32 = 9; // UHDM extension

pub const vpiBlocking: PLI_INT32 = 41;
pub const vpiCaseType: PLI_INT32 = 42;
pub const vpiCaseExact: PLI_INT32 = 1; // vpiCaseType subtypes
pub const vpiCaseX: PLI_INT32 = 2;
pub const vpiCaseZ: PLI_INT32 = 3;
pub const vpiNetDeclAssign: PLI_INT32 = 43;

// Task / function properties
pub const vpiFuncType: PLI_INT32 = 44;
pub const vpiIntFunc: PLI_INT32 = 1; // vpiFuncType subtypes
pub const vpiRealFunc: PLI_INT32 = 2;
pub const vpiTimeFunc: PLI_INT32 = 3;
pub const vpiSizedFunc: PLI_INT32 = 4;
pub const vpiSizedSignedFunc: PLI_INT32 = 5;
// 1364-1995 aliases
pub const vpiSysFuncType: PLI_INT32 = vpiFuncType;
pub const vpiSysFuncInt: PLI_INT32 = vpiIntFunc;
pub const vpiSysFuncReal: PLI_INT32 = vpiRealFunc;
pub const vpiSysFuncTime: PLI_INT32 = vpiTimeFunc;
pub const vpiSysFuncSized: PLI_INT32 = vpiSizedFunc;

pub const vpiUserDefn: PLI_INT32 = 45;
pub const vpiScheduled: PLI_INT32 = 46;

// 1364-2001 properties
pub const vpiActive: PLI_INT32 = 49;
pub const vpiAutomatic: PLI_INT32 = 50;
pub const vpiCell: PLI_INT32 = 51;
pub const vpiConfig: PLI_INT32 = 52;
pub const vpiConstantSelect: PLI_INT32 = 53;
pub const vpiDecompile: PLI_INT32 = 54;
pub const vpiDefAttribute: PLI_INT32 = 55;
pub const vpiDelayType: PLI_INT32 = 56;
pub const vpiModPathDelay: PLI_INT32 = 1; // vpiDelayType subtypes
pub const vpiInterModPathDelay: PLI_INT32 = 2;
pub const vpiMIPDelay: PLI_INT32 = 3;
pub const vpiIteratorType: PLI_INT32 = 57;
pub const vpiLibrary: PLI_INT32 = 58;
pub const vpiOffset: PLI_INT32 = 60;
pub const vpiResolvedNetType: PLI_INT32 = 61;
pub const vpiSaveRestartID: PLI_INT32 = 62;
pub const vpiSaveRestartLocation: PLI_INT32 = 63;
pub const vpiValid: PLI_INT32 = 64;
pub const vpiValidFalse: PLI_INT32 = 0;
pub const vpiValidTrue: PLI_INT32 = 1;
pub const vpiSigned: PLI_INT32 = 65;
pub const vpiLocalParam: PLI_INT32 = 70;
pub const vpiModPathHasIfNone: PLI_INT32 = 71;

// 1364-2005 properties
pub const vpiIndexedPartSelectType: PLI_INT32 = 72;
pub const vpiPosIndexed: PLI_INT32 = 1;
pub const vpiNegIndexed: PLI_INT32 = 2;
pub const vpiIsMemory: PLI_INT32 = 73;
pub const vpiIsProtected: PLI_INT32 = 74;

// vpi_control() constants (1364-2001)
pub const vpiStop: PLI_INT32 = 66;
pub const vpiFinish: PLI_INT32 = 67;
pub const vpiReset: PLI_INT32 = 68;
pub const vpiSetInteractiveScope: PLI_INT32 = 69;

// I/O
pub const VPI_MCD_STDOUT: u32 = 0x0000_0001;

// ── Strength values ───────────────────────────────────────────────────────────

pub const vpiSupplyDrive: PLI_INT32 = 0x80;
pub const vpiStrongDrive: PLI_INT32 = 0x40;
pub const vpiPullDrive: PLI_INT32 = 0x20;
pub const vpiWeakDrive: PLI_INT32 = 0x08;
pub const vpiLargeCharge: PLI_INT32 = 0x10;
pub const vpiMediumCharge: PLI_INT32 = 0x04;
pub const vpiSmallCharge: PLI_INT32 = 0x02;
pub const vpiHiZ: PLI_INT32 = 0x01;

// ── Time types ────────────────────────────────────────────────────────────────

pub const vpiScaledRealTime: PLI_INT32 = 1;
pub const vpiSimTime: PLI_INT32 = 2;
pub const vpiSuppressTime: PLI_INT32 = 3;

// ── Value formats ─────────────────────────────────────────────────────────────

pub const vpiBinStrVal: PLI_INT32 = 1;
pub const vpiOctStrVal: PLI_INT32 = 2;
pub const vpiDecStrVal: PLI_INT32 = 3;
pub const vpiHexStrVal: PLI_INT32 = 4;
pub const vpiScalarVal: PLI_INT32 = 5;
pub const vpiIntVal: PLI_INT32 = 6;
pub const vpiRealVal: PLI_INT32 = 7;
pub const vpiStringVal: PLI_INT32 = 8;
pub const vpiVectorVal: PLI_INT32 = 9;
pub const vpiStrengthVal: PLI_INT32 = 10;
pub const vpiTimeVal: PLI_INT32 = 11;
pub const vpiObjTypeVal: PLI_INT32 = 12;
pub const vpiSuppressVal: PLI_INT32 = 13;
pub const vpiShortIntVal: PLI_INT32 = 14;
pub const vpiLongIntVal: PLI_INT32 = 15;
pub const vpiShortRealVal: PLI_INT32 = 16;
pub const vpiRawTwoStateVal: PLI_INT32 = 17;
pub const vpiRawFourStateVal: PLI_INT32 = 18;
pub const vpiUIntVal: PLI_INT32 = 19; // UHDM extension

// ── Delay modes ───────────────────────────────────────────────────────────────

pub const vpiNoDelay: PLI_INT32 = 1;
pub const vpiInertialDelay: PLI_INT32 = 2;
pub const vpiTransportDelay: PLI_INT32 = 3;
pub const vpiPureTransportDelay: PLI_INT32 = 4;

// ── put_value flags ───────────────────────────────────────────────────────────

pub const vpiForceFlag: PLI_INT32 = 5;
pub const vpiReleaseFlag: PLI_INT32 = 6;
pub const vpiCancelEvent: PLI_INT32 = 7;
pub const vpiReturnEvent: PLI_INT32 = 0x1000;
pub const vpiUserAllocFlag: PLI_INT32 = 0x2000;
pub const vpiOneValue: PLI_INT32 = 0x4000;
pub const vpiPropagateOff: PLI_INT32 = 0x8000;

// ── Scalar values ─────────────────────────────────────────────────────────────

pub const vpi0: PLI_INT32 = 0;
pub const vpi1: PLI_INT32 = 1;
pub const vpiZ: PLI_INT32 = 2;
pub const vpiX: PLI_INT32 = 3;
pub const vpiH: PLI_INT32 = 4;
pub const vpiL: PLI_INT32 = 5;
pub const vpiDontCare: PLI_INT32 = 6;

// ── System task / function subtypes ──────────────────────────────────────────

pub const vpiSysTask: PLI_INT32 = 1;
pub const vpiSysFunc: PLI_INT32 = 2;

// ── Error state and severity ──────────────────────────────────────────────────

pub const vpiCompile: PLI_INT32 = 1;
pub const vpiPLI: PLI_INT32 = 2;
pub const vpiRun: PLI_INT32 = 3;

pub const vpiNotice: PLI_INT32 = 1;
pub const vpiWarning: PLI_INT32 = 2;
pub const vpiError: PLI_INT32 = 3;
pub const vpiSystem: PLI_INT32 = 4;
pub const vpiInternal: PLI_INT32 = 5;

// ── Callback reasons ──────────────────────────────────────────────────────────

pub const cbValueChange: PLI_INT32 = 1;
pub const cbStmt: PLI_INT32 = 2;
pub const cbForce: PLI_INT32 = 3;
pub const cbRelease: PLI_INT32 = 4;
pub const cbAtStartOfSimTime: PLI_INT32 = 5;
pub const cbReadWriteSynch: PLI_INT32 = 6;
pub const cbReadOnlySynch: PLI_INT32 = 7;
pub const cbNextSimTime: PLI_INT32 = 8;
pub const cbAfterDelay: PLI_INT32 = 9;
pub const cbEndOfCompile: PLI_INT32 = 10;
pub const cbStartOfSimulation: PLI_INT32 = 11;
pub const cbEndOfSimulation: PLI_INT32 = 12;
pub const cbError: PLI_INT32 = 13;
pub const cbTchkViolation: PLI_INT32 = 14;
pub const cbStartOfSave: PLI_INT32 = 15;
pub const cbEndOfSave: PLI_INT32 = 16;
pub const cbStartOfRestart: PLI_INT32 = 17;
pub const cbEndOfRestart: PLI_INT32 = 18;
pub const cbStartOfReset: PLI_INT32 = 19;
pub const cbEndOfReset: PLI_INT32 = 20;
pub const cbEnterInteractive: PLI_INT32 = 21;
pub const cbExitInteractive: PLI_INT32 = 22;
pub const cbInteractiveScopeChange: PLI_INT32 = 23;
pub const cbUnresolvedSystf: PLI_INT32 = 24;
pub const cbAssign: PLI_INT32 = 25;
pub const cbDeassign: PLI_INT32 = 26;
pub const cbDisable: PLI_INT32 = 27;
pub const cbPLIError: PLI_INT32 = 28;
pub const cbSignal: PLI_INT32 = 29;
pub const cbNBASynch: PLI_INT32 = 30;
pub const cbAtEndOfSimTime: PLI_INT32 = 31;

// ── uhdm_vpi_user.h — UHDM extension constants ───────────────────────────────

pub const vpiDesign: PLI_INT32 = 3000;
pub const vpiInterfaceTypespec: PLI_INT32 = 3001;
pub const vpiNets: PLI_INT32 = 3002;
pub const vpiSimpleExpr: PLI_INT32 = 3003;
pub const vpiParameters: PLI_INT32 = 3004;
pub const vpiSequenceExpr: PLI_INT32 = 3005;
pub const vpiSoftDisable: PLI_INT32 = 3006;
pub const vpiIsModPort: PLI_INT32 = 3007;
pub const vpiVarBit: PLI_INT32 = 3008;
pub const vpiLogicVar: PLI_INT32 = 3009;
pub const vpiArrayVar: PLI_INT32 = 3010;
pub const vpiWaits: PLI_INT32 = 3011;
pub const vpiDisables: PLI_INT32 = 3012;
pub const vpiStructMember: PLI_INT32 = 3013;
pub const vpiImported: PLI_INT32 = 3014;
pub const vpiColumnNo: PLI_INT32 = 3015;
pub const vpiEndLineNo: PLI_INT32 = 3016;
pub const vpiEndColumnNo: PLI_INT32 = 3017;
pub const vpiRefFile: PLI_INT32 = 3018;
pub const vpiRefLineNo: PLI_INT32 = 3019;
pub const vpiRefColumnNo: PLI_INT32 = 3020;
pub const vpiRefEndLineNo: PLI_INT32 = 3021;
pub const vpiRefEndColumnNo: PLI_INT32 = 3022;
pub const vpiIncludeFileInfo: PLI_INT32 = 3023;
pub const vpiIncludedFile: PLI_INT32 = 3024;

pub const vpiUnsupportedStmt: PLI_INT32 = 4000;
pub const vpiUnsupportedExpr: PLI_INT32 = 4001;
pub const vpiUnsupportedTypespec: PLI_INT32 = 4002;

pub const vpiHierPath: PLI_INT32 = 5000;
pub const vpiReordered: PLI_INT32 = 5001;
pub const vpiElaborated: PLI_INT32 = 5002;
pub const vpiRefVar: PLI_INT32 = 5003;
pub const vpiOverriden: PLI_INT32 = 5004;
pub const vpiFlattened: PLI_INT32 = 5005;
pub const vpiCheckerDecl: PLI_INT32 = 5006;
pub const vpiCheckerInst: PLI_INT32 = 5007;
pub const vpiCheckerPort: PLI_INT32 = 5008;
pub const vpiCheckerInstPort: PLI_INT32 = 5009;
pub const vpiArrayExpr: PLI_INT32 = 5010;
pub const vpiRefModule: PLI_INT32 = 5011;
pub const vpiGenStmt: PLI_INT32 = 5012;
pub const vpiGenIf: PLI_INT32 = 5013;
pub const vpiGenIfElse: PLI_INT32 = 5014;
pub const vpiGenFor: PLI_INT32 = 5015;
pub const vpiGenCase: PLI_INT32 = 5016;
pub const vpiGenRegion: PLI_INT32 = 5017;

// ── uhdm_types.h — UHDM_OBJECT_TYPE enum values ──────────────────────────────

pub const uhdmactual: PLI_INT32 = 2001;
pub const uhdmactual_group: PLI_INT32 = 2002;
pub const uhdmactual_typespec: PLI_INT32 = 2003;
pub const uhdmactual_value: PLI_INT32 = 2004;
pub const uhdmalias_stmt: PLI_INT32 = 2005;
pub const uhdmalias_stmts: PLI_INT32 = 2006;
pub const uhdmallClasses: PLI_INT32 = 2007;
pub const uhdmallInterfaces: PLI_INT32 = 2008;
pub const uhdmallModules: PLI_INT32 = 2009;
pub const uhdmallPackages: PLI_INT32 = 2010;
pub const uhdmallPrograms: PLI_INT32 = 2011;
pub const uhdmallUdps: PLI_INT32 = 2012;
pub const uhdmalways: PLI_INT32 = 2013;
pub const uhdmany_pattern: PLI_INT32 = 2014;
pub const uhdmarguments: PLI_INT32 = 2015;
pub const uhdmarray_expr: PLI_INT32 = 2016;
pub const uhdmarray_net: PLI_INT32 = 2017;
pub const uhdmarray_nets: PLI_INT32 = 2018;
pub const uhdmarray_typespec: PLI_INT32 = 2019;
pub const uhdmarray_var: PLI_INT32 = 2020;
pub const uhdmarray_var_mems: PLI_INT32 = 2021;
pub const uhdmarray_vars: PLI_INT32 = 2022;
pub const uhdmassert_stmt: PLI_INT32 = 2023;
pub const uhdmassertion: PLI_INT32 = 2024;
pub const uhdmassertions: PLI_INT32 = 2025;
pub const uhdmassign_stmt: PLI_INT32 = 2026;
pub const uhdmassignment: PLI_INT32 = 2027;
pub const uhdmassume: PLI_INT32 = 2028;
pub const uhdmatomic_stmt: PLI_INT32 = 2029;
pub const uhdmattribute: PLI_INT32 = 2030;
pub const uhdmattributes: PLI_INT32 = 2031;
pub const uhdmbase_expr: PLI_INT32 = 2032;
pub const uhdmbase_typespec: PLI_INT32 = 2033;
pub const uhdmbegin: PLI_INT32 = 2034;
pub const uhdmbit_select: PLI_INT32 = 2035;
pub const uhdmbit_typespec: PLI_INT32 = 2036;
pub const uhdmbit_var: PLI_INT32 = 2037;
pub const uhdmbits: PLI_INT32 = 2038;
pub const uhdmbreak_stmt: PLI_INT32 = 2039;
pub const uhdmbyte_typespec: PLI_INT32 = 2040;
pub const uhdmbyte_var: PLI_INT32 = 2041;
pub const uhdmcase_item: PLI_INT32 = 2042;
pub const uhdmcase_items: PLI_INT32 = 2043;
pub const uhdmcase_property: PLI_INT32 = 2044;
pub const uhdmcase_property_item: PLI_INT32 = 2045;
pub const uhdmcase_property_items: PLI_INT32 = 2046;
pub const uhdmcase_stmt: PLI_INT32 = 2047;
pub const uhdmcast_to_expr: PLI_INT32 = 2048;
pub const uhdmchandle_typespec: PLI_INT32 = 2049;
pub const uhdmchandle_var: PLI_INT32 = 2050;
pub const uhdmchecker_decl: PLI_INT32 = 2051;
pub const uhdmchecker_inst: PLI_INT32 = 2052;
pub const uhdmchecker_inst_port: PLI_INT32 = 2053;
pub const uhdmchecker_port: PLI_INT32 = 2054;
pub const uhdmclass_defn: PLI_INT32 = 2055;
pub const uhdmclass_defns: PLI_INT32 = 2056;
pub const uhdmclass_obj: PLI_INT32 = 2057;
pub const uhdmclass_typespec: PLI_INT32 = 2058;
pub const uhdmclass_typespecs: PLI_INT32 = 2059;
pub const uhdmclass_var: PLI_INT32 = 2060;
pub const uhdmclocked_property: PLI_INT32 = 2061;
pub const uhdmclocked_seq: PLI_INT32 = 2062;
pub const uhdmclocked_seqs: PLI_INT32 = 2063;
pub const uhdmclocking_block: PLI_INT32 = 2064;
pub const uhdmclocking_blocks: PLI_INT32 = 2065;
pub const uhdmclocking_event: PLI_INT32 = 2066;
pub const uhdmclocking_io_decl: PLI_INT32 = 2067;
pub const uhdmclocking_io_decls: PLI_INT32 = 2068;
pub const uhdmconcurrent_assertions: PLI_INT32 = 2069;
pub const uhdmcondition: PLI_INT32 = 2070;
pub const uhdmconstant: PLI_INT32 = 2071;
pub const uhdmconstr_foreach: PLI_INT32 = 2072;
pub const uhdmconstr_if: PLI_INT32 = 2073;
pub const uhdmconstr_if_else: PLI_INT32 = 2074;
pub const uhdmconstraint: PLI_INT32 = 2075;
pub const uhdmconstraint_expr: PLI_INT32 = 2076;
pub const uhdmconstraint_exprs: PLI_INT32 = 2077;
pub const uhdmconstraint_item_group: PLI_INT32 = 2078;
pub const uhdmconstraint_items: PLI_INT32 = 2079;
pub const uhdmconstraint_ordering: PLI_INT32 = 2080;
pub const uhdmconstraints: PLI_INT32 = 2081;
pub const uhdmcont_assign: PLI_INT32 = 2082;
pub const uhdmcont_assign_bit: PLI_INT32 = 2083;
pub const uhdmcont_assign_bits: PLI_INT32 = 2084;
pub const uhdmcont_assigns: PLI_INT32 = 2085;
pub const uhdmcontinue_stmt: PLI_INT32 = 2086;
pub const uhdmcover: PLI_INT32 = 2087;
pub const uhdmdeassign: PLI_INT32 = 2088;
pub const uhdmdef_param: PLI_INT32 = 2089;
pub const uhdmdef_params: PLI_INT32 = 2090;
pub const uhdmdefault_clocking: PLI_INT32 = 2091;
pub const uhdmdefault_value: PLI_INT32 = 2092;
pub const uhdmdelay: PLI_INT32 = 2093;
pub const uhdmdelay_control: PLI_INT32 = 2094;
pub const uhdmdelay_term: PLI_INT32 = 2095;
pub const uhdmderiveds: PLI_INT32 = 2096;
pub const uhdmdesign: PLI_INT32 = 2097;
pub const uhdmdisable: PLI_INT32 = 2098;
pub const uhdmdisable_fork: PLI_INT32 = 2099;
pub const uhdmdisables: PLI_INT32 = 2100;
pub const uhdmdist_item: PLI_INT32 = 2101;
pub const uhdmdist_items: PLI_INT32 = 2102;
pub const uhdmdistribution: PLI_INT32 = 2103;
pub const uhdmdo_while: PLI_INT32 = 2104;
pub const uhdmdrivers: PLI_INT32 = 2105;
pub const uhdmelab_tasks: PLI_INT32 = 2106;
pub const uhdmelem_typespec: PLI_INT32 = 2107;
pub const uhdmelements: PLI_INT32 = 2108;
pub const uhdmelse_constraint_exprs: PLI_INT32 = 2109;
pub const uhdmelse_stmt: PLI_INT32 = 2110;
pub const uhdmenum_const: PLI_INT32 = 2111;
pub const uhdmenum_consts: PLI_INT32 = 2112;
pub const uhdmenum_net: PLI_INT32 = 2113;
pub const uhdmenum_struct_packed_net_group: PLI_INT32 = 2114;
pub const uhdmenum_struct_union_packed_array_typespec_group: PLI_INT32 = 2115;
pub const uhdmenum_struct_union_packed_var_group: PLI_INT32 = 2116;
pub const uhdmenum_typespec: PLI_INT32 = 2117;
pub const uhdmenum_var: PLI_INT32 = 2118;
pub const uhdmevent_control: PLI_INT32 = 2119;
pub const uhdmevent_stmt: PLI_INT32 = 2120;
pub const uhdmevent_typespec: PLI_INT32 = 2121;
pub const uhdmexpect_stmt: PLI_INT32 = 2122;
pub const uhdmexpr: PLI_INT32 = 2123;
pub const uhdmexpr_constr_group: PLI_INT32 = 2124;
pub const uhdmexpr_dist: PLI_INT32 = 2125;
pub const uhdmexpr_index: PLI_INT32 = 2126;
pub const uhdmexpr_indexes: PLI_INT32 = 2127;
pub const uhdmexpr_interf_expr_group: PLI_INT32 = 2128;
pub const uhdmexpr_range_group: PLI_INT32 = 2129;
pub const uhdmexpr_ref_obj_group: PLI_INT32 = 2130;
pub const uhdmexpr_sequence_inst_group: PLI_INT32 = 2131;
pub const uhdmexpr_sequence_inst_named_event_group: PLI_INT32 = 2132;
pub const uhdmexpr_tchk_term_group: PLI_INT32 = 2133;
pub const uhdmexpr_tchk_terms: PLI_INT32 = 2134;
pub const uhdmexpr_typespec_group: PLI_INT32 = 2135;
pub const uhdmexpressions: PLI_INT32 = 2136;
pub const uhdmexprs: PLI_INT32 = 2137;
pub const uhdmextends: PLI_INT32 = 2138;
pub const uhdmfinal_stmt: PLI_INT32 = 2139;
pub const uhdmfor_stmt: PLI_INT32 = 2140;
pub const uhdmforce: PLI_INT32 = 2141;
pub const uhdmforeach_stmt: PLI_INT32 = 2142;
pub const uhdmforever_stmt: PLI_INT32 = 2143;
pub const uhdmfork_stmt: PLI_INT32 = 2144;
pub const uhdmfunc_call: PLI_INT32 = 2145;
pub const uhdmfunction: PLI_INT32 = 2146;
pub const uhdmfunctions: PLI_INT32 = 2147;
pub const uhdmgate: PLI_INT32 = 2148;
pub const uhdmgate_array: PLI_INT32 = 2149;
pub const uhdmgen_case: PLI_INT32 = 2150;
pub const uhdmgen_for: PLI_INT32 = 2151;
pub const uhdmgen_if: PLI_INT32 = 2152;
pub const uhdmgen_if_else: PLI_INT32 = 2153;
pub const uhdmgen_region: PLI_INT32 = 2154;
pub const uhdmgen_scope: PLI_INT32 = 2155;
pub const uhdmgen_scope_array: PLI_INT32 = 2156;
pub const uhdmgen_scope_arrays: PLI_INT32 = 2157;
pub const uhdmgen_scopes: PLI_INT32 = 2158;
pub const uhdmgen_stmt: PLI_INT32 = 2159;
pub const uhdmgen_stmts: PLI_INT32 = 2160;
pub const uhdmgen_var: PLI_INT32 = 2161;
pub const uhdmglobal_clocking: PLI_INT32 = 2162;
pub const uhdmhier_path: PLI_INT32 = 2163;
pub const uhdmhigh_conn: PLI_INT32 = 2164;
pub const uhdmif_else: PLI_INT32 = 2165;
pub const uhdmif_stmt: PLI_INT32 = 2166;
pub const uhdmimmediate_assert: PLI_INT32 = 2167;
pub const uhdmimmediate_assume: PLI_INT32 = 2168;
pub const uhdmimmediate_cover: PLI_INT32 = 2169;
pub const uhdmimplication: PLI_INT32 = 2170;
pub const uhdmimport_typespec: PLI_INT32 = 2171;
pub const uhdminclude_file_info: PLI_INT32 = 2172;
pub const uhdminclude_file_infos: PLI_INT32 = 2173;
pub const uhdmindex: PLI_INT32 = 2174;
pub const uhdmindex_typespec: PLI_INT32 = 2175;
pub const uhdmindexed_part_select: PLI_INT32 = 2176;
pub const uhdmindexes: PLI_INT32 = 2177;
pub const uhdminitial: PLI_INT32 = 2178;
pub const uhdminput_skew: PLI_INT32 = 2179;
pub const uhdminstance: PLI_INT32 = 2180;
pub const uhdminstance_array: PLI_INT32 = 2181;
pub const uhdminstance_item: PLI_INT32 = 2182;
pub const uhdminstance_items: PLI_INT32 = 2183;
pub const uhdminstances: PLI_INT32 = 2184;
pub const uhdmint_typespec: PLI_INT32 = 2185;
pub const uhdmint_var: PLI_INT32 = 2186;
pub const uhdminteger_net: PLI_INT32 = 2187;
pub const uhdminteger_typespec: PLI_INT32 = 2188;
pub const uhdminteger_var: PLI_INT32 = 2189;
pub const uhdminterf_prog_mod_group: PLI_INT32 = 2190;
pub const uhdminterface_array: PLI_INT32 = 2191;
pub const uhdminterface_arrays: PLI_INT32 = 2192;
pub const uhdminterface_expr: PLI_INT32 = 2193;
pub const uhdminterface_inst: PLI_INT32 = 2194;
pub const uhdminterface_tf_decl: PLI_INT32 = 2195;
pub const uhdminterface_tf_decls: PLI_INT32 = 2196;
pub const uhdminterface_typespec: PLI_INT32 = 2197;
pub const uhdminterfaces: PLI_INT32 = 2198;
pub const uhdmio_decl: PLI_INT32 = 2199;
pub const uhdmio_decls: PLI_INT32 = 2200;
pub const uhdmitem: PLI_INT32 = 2201;
pub const uhdmleft_expr: PLI_INT32 = 2202;
pub const uhdmleft_range: PLI_INT32 = 2203;
pub const uhdmlet_decl: PLI_INT32 = 2204;
pub const uhdmlet_decls: PLI_INT32 = 2205;
pub const uhdmlet_expr: PLI_INT32 = 2206;
pub const uhdmlhs: PLI_INT32 = 2207;
pub const uhdmloads: PLI_INT32 = 2208;
pub const uhdmlocal_drivers: PLI_INT32 = 2209;
pub const uhdmlocal_loads: PLI_INT32 = 2210;
pub const uhdmlogic_net: PLI_INT32 = 2211;
pub const uhdmlogic_typespec: PLI_INT32 = 2212;
pub const uhdmlogic_var: PLI_INT32 = 2213;
pub const uhdmlogic_vars: PLI_INT32 = 2214;
pub const uhdmlong_int_typespec: PLI_INT32 = 2215;
pub const uhdmlong_int_var: PLI_INT32 = 2216;
pub const uhdmlow_conn: PLI_INT32 = 2217;
pub const uhdmmembers: PLI_INT32 = 2218;
pub const uhdmmessages: PLI_INT32 = 2219;
pub const uhdmmethod_func_call: PLI_INT32 = 2220;
pub const uhdmmethod_func_task_call_group: PLI_INT32 = 2221;
pub const uhdmmethod_task_call: PLI_INT32 = 2222;
pub const uhdmmod_path: PLI_INT32 = 2223;
pub const uhdmmod_paths: PLI_INT32 = 2224;
pub const uhdmmodport: PLI_INT32 = 2225;
pub const uhdmmodports: PLI_INT32 = 2226;
pub const uhdmmodule_array: PLI_INT32 = 2227;
pub const uhdmmodule_arrays: PLI_INT32 = 2228;
pub const uhdmmodule_inst: PLI_INT32 = 2229;
pub const uhdmmodule_typespec: PLI_INT32 = 2230;
pub const uhdmmodules: PLI_INT32 = 2231;
pub const uhdmmulticlock_sequence_expr: PLI_INT32 = 2232;
pub const uhdmnamed_begin: PLI_INT32 = 2233;
pub const uhdmnamed_event: PLI_INT32 = 2234;
pub const uhdmnamed_event_array: PLI_INT32 = 2235;
pub const uhdmnamed_event_arrays: PLI_INT32 = 2236;
pub const uhdmnamed_event_sequence_expr_group: PLI_INT32 = 2237;
pub const uhdmnamed_event_sequence_expr_groups: PLI_INT32 = 2238;
pub const uhdmnamed_events: PLI_INT32 = 2239;
pub const uhdmnamed_fork: PLI_INT32 = 2240;
pub const uhdmnet: PLI_INT32 = 2241;
pub const uhdmnet_bit: PLI_INT32 = 2242;
pub const uhdmnet_bits: PLI_INT32 = 2243;
pub const uhdmnet_drivers: PLI_INT32 = 2244;
pub const uhdmnet_loads: PLI_INT32 = 2245;
pub const uhdmnets: PLI_INT32 = 2246;
pub const uhdmnets_vars_ref_obj_group: PLI_INT32 = 2247;
pub const uhdmnull_stmt: PLI_INT32 = 2248;
pub const uhdmoperand_group: PLI_INT32 = 2249;
pub const uhdmoperands: PLI_INT32 = 2250;
pub const uhdmoperation: PLI_INT32 = 2251;
pub const uhdmordered_wait: PLI_INT32 = 2252;
pub const uhdmoutput_skew: PLI_INT32 = 2253;
pub const uhdmpackage: PLI_INT32 = 2254;
pub const uhdmpacked_array_net: PLI_INT32 = 2255;
pub const uhdmpacked_array_typespec: PLI_INT32 = 2256;
pub const uhdmpacked_array_var: PLI_INT32 = 2257;
pub const uhdmparam_assign: PLI_INT32 = 2258;
pub const uhdmparam_assigns: PLI_INT32 = 2259;
pub const uhdmparameter: PLI_INT32 = 2260;
pub const uhdmparameters: PLI_INT32 = 2261;
pub const uhdmpart_select: PLI_INT32 = 2262;
pub const uhdmpath_elems: PLI_INT32 = 2263;
pub const uhdmpath_term: PLI_INT32 = 2264;
pub const uhdmpath_terms: PLI_INT32 = 2265;
pub const uhdmpattern: PLI_INT32 = 2266;
pub const uhdmpattern_expr_group: PLI_INT32 = 2267;
pub const uhdmport: PLI_INT32 = 2268;
pub const uhdmport_bit: PLI_INT32 = 2269;
pub const uhdmports: PLI_INT32 = 2270;
pub const uhdmprefix: PLI_INT32 = 2271;
pub const uhdmprim_term: PLI_INT32 = 2272;
pub const uhdmprim_terms: PLI_INT32 = 2273;
pub const uhdmprimitive: PLI_INT32 = 2274;
pub const uhdmprimitive_array: PLI_INT32 = 2275;
pub const uhdmprimitive_arrays: PLI_INT32 = 2276;
pub const uhdmprimitives: PLI_INT32 = 2277;
pub const uhdmprocess: PLI_INT32 = 2278;
pub const uhdmprocess_stmt: PLI_INT32 = 2279;
pub const uhdmprogram: PLI_INT32 = 2280;
pub const uhdmprogram_array: PLI_INT32 = 2281;
pub const uhdmprogram_arrays: PLI_INT32 = 2282;
pub const uhdmprograms: PLI_INT32 = 2283;
pub const uhdmprop_formal_decl: PLI_INT32 = 2284;
pub const uhdmprop_formal_decls: PLI_INT32 = 2285;
pub const uhdmproperty: PLI_INT32 = 2286;
pub const uhdmproperty_decl: PLI_INT32 = 2287;
pub const uhdmproperty_decls: PLI_INT32 = 2288;
pub const uhdmproperty_expr: PLI_INT32 = 2289;
pub const uhdmproperty_expr_group: PLI_INT32 = 2290;
pub const uhdmproperty_expr_named_event_group: PLI_INT32 = 2291;
pub const uhdmproperty_inst: PLI_INT32 = 2292;
pub const uhdmproperty_inst_spec_group: PLI_INT32 = 2293;
pub const uhdmproperty_spec: PLI_INT32 = 2294;
pub const uhdmproperty_typespec: PLI_INT32 = 2295;
pub const uhdmrange: PLI_INT32 = 2296;
pub const uhdmranges: PLI_INT32 = 2297;
pub const uhdmreal_typespec: PLI_INT32 = 2298;
pub const uhdmreal_var: PLI_INT32 = 2299;
pub const uhdmref_module: PLI_INT32 = 2300;
pub const uhdmref_modules: PLI_INT32 = 2301;
pub const uhdmref_obj: PLI_INT32 = 2302;
pub const uhdmref_obj_interf_net_var_group: PLI_INT32 = 2303;
pub const uhdmref_typespec: PLI_INT32 = 2304;
pub const uhdmref_var: PLI_INT32 = 2305;
pub const uhdmreg: PLI_INT32 = 2306;
pub const uhdmreg_array: PLI_INT32 = 2307;
pub const uhdmregs: PLI_INT32 = 2308;
pub const uhdmrelease: PLI_INT32 = 2309;
pub const uhdmrepeat: PLI_INT32 = 2310;
pub const uhdmrepeat_control: PLI_INT32 = 2311;
pub const uhdmresolution_func: PLI_INT32 = 2312;
pub const uhdmrestrict: PLI_INT32 = 2313;
pub const uhdmreturn: PLI_INT32 = 2314;
pub const uhdmreturn_stmt: PLI_INT32 = 2315;
pub const uhdmrhs: PLI_INT32 = 2316;
pub const uhdmright_expr: PLI_INT32 = 2317;
pub const uhdmright_range: PLI_INT32 = 2318;
pub const uhdmroot_value: PLI_INT32 = 2319;
pub const uhdmscope: PLI_INT32 = 2320;
pub const uhdmscopes: PLI_INT32 = 2321;
pub const uhdmseq_formal_decl: PLI_INT32 = 2322;
pub const uhdmseq_formal_decls: PLI_INT32 = 2323;
pub const uhdmsequence: PLI_INT32 = 2324;
pub const uhdmsequence_decl: PLI_INT32 = 2325;
pub const uhdmsequence_decls: PLI_INT32 = 2326;
pub const uhdmsequence_expr_group: PLI_INT32 = 2327;
pub const uhdmsequence_expr_multiclock_group: PLI_INT32 = 2328;
pub const uhdmsequence_inst: PLI_INT32 = 2329;
pub const uhdmsequence_typespec: PLI_INT32 = 2330;
pub const uhdmshort_int_typespec: PLI_INT32 = 2331;
pub const uhdmshort_int_var: PLI_INT32 = 2332;
pub const uhdmshort_real_typespec: PLI_INT32 = 2333;
pub const uhdmshort_real_var: PLI_INT32 = 2334;
pub const uhdmsim_net: PLI_INT32 = 2335;
pub const uhdmsimple_expr: PLI_INT32 = 2336;
pub const uhdmsimple_expr_use_group: PLI_INT32 = 2337;
pub const uhdmsoft_disable: PLI_INT32 = 2338;
pub const uhdmsolve_afters: PLI_INT32 = 2339;
pub const uhdmsolve_befores: PLI_INT32 = 2340;
pub const uhdmspec_param: PLI_INT32 = 2341;
pub const uhdmspec_params: PLI_INT32 = 2342;
pub const uhdmstmt: PLI_INT32 = 2343;
pub const uhdmstmts: PLI_INT32 = 2344;
pub const uhdmstring_typespec: PLI_INT32 = 2345;
pub const uhdmstring_var: PLI_INT32 = 2346;
pub const uhdmstruct_net: PLI_INT32 = 2347;
pub const uhdmstruct_pattern: PLI_INT32 = 2348;
pub const uhdmstruct_typespec: PLI_INT32 = 2349;
pub const uhdmstruct_var: PLI_INT32 = 2350;
pub const uhdmswitch_array: PLI_INT32 = 2351;
pub const uhdmswitch_tran: PLI_INT32 = 2352;
pub const uhdmsys_func_call: PLI_INT32 = 2353;
pub const uhdmsys_func_task_call_group: PLI_INT32 = 2354;
pub const uhdmsys_task_call: PLI_INT32 = 2355;
pub const uhdmtable_entry: PLI_INT32 = 2356;
pub const uhdmtable_entrys: PLI_INT32 = 2357;
pub const uhdmtagged_pattern: PLI_INT32 = 2358;
pub const uhdmtask: PLI_INT32 = 2359;
pub const uhdmtask_call: PLI_INT32 = 2360;
pub const uhdmtask_func: PLI_INT32 = 2361;
pub const uhdmtask_func_named_begin_fork_group: PLI_INT32 = 2362;
pub const uhdmtask_funcs: PLI_INT32 = 2363;
pub const uhdmtasks: PLI_INT32 = 2364;
pub const uhdmtchk: PLI_INT32 = 2365;
pub const uhdmtchk_data_term: PLI_INT32 = 2366;
pub const uhdmtchk_ref_term: PLI_INT32 = 2367;
pub const uhdmtchk_term: PLI_INT32 = 2368;
pub const uhdmtchk_terms: PLI_INT32 = 2369;
pub const uhdmtchks: PLI_INT32 = 2370;
pub const uhdmtf_call: PLI_INT32 = 2371;
pub const uhdmtf_call_args: PLI_INT32 = 2372;
pub const uhdmthread_obj: PLI_INT32 = 2373;
pub const uhdmthreads: PLI_INT32 = 2374;
pub const uhdmtime_net: PLI_INT32 = 2375;
pub const uhdmtime_typespec: PLI_INT32 = 2376;
pub const uhdmtime_var: PLI_INT32 = 2377;
pub const uhdmtopModules: PLI_INT32 = 2378;
pub const uhdmtopPackages: PLI_INT32 = 2379;
pub const uhdmtype_parameter: PLI_INT32 = 2380;
pub const uhdmtypedef_alias: PLI_INT32 = 2381;
pub const uhdmtypespec: PLI_INT32 = 2382;
pub const uhdmtypespec_member: PLI_INT32 = 2383;
pub const uhdmtypespecs: PLI_INT32 = 2384;
pub const uhdmudp: PLI_INT32 = 2385;
pub const uhdmudp_array: PLI_INT32 = 2386;
pub const uhdmudp_defn: PLI_INT32 = 2387;
pub const uhdmunion_typespec: PLI_INT32 = 2388;
pub const uhdmunion_var: PLI_INT32 = 2389;
pub const uhdmunsupported_expr: PLI_INT32 = 2390;
pub const uhdmunsupported_stmt: PLI_INT32 = 2391;
pub const uhdmunsupported_typespec: PLI_INT32 = 2392;
pub const uhdmuser_systf: PLI_INT32 = 2393;
pub const uhdmvalue_range: PLI_INT32 = 2394;
pub const uhdmvar_bit: PLI_INT32 = 2395;
pub const uhdmvar_bits: PLI_INT32 = 2396;
pub const uhdmvar_select: PLI_INT32 = 2397;
pub const uhdmvar_selects: PLI_INT32 = 2398;
pub const uhdmvariable: PLI_INT32 = 2399;
pub const uhdmvariable_drivers: PLI_INT32 = 2400;
pub const uhdmvariable_drivers_group: PLI_INT32 = 2401;
pub const uhdmvariable_loads: PLI_INT32 = 2402;
pub const uhdmvariable_loads_group: PLI_INT32 = 2403;
pub const uhdmvariables: PLI_INT32 = 2404;
pub const uhdmvariables_operation_group: PLI_INT32 = 2405;
pub const uhdmvirtual_interface_var: PLI_INT32 = 2406;
pub const uhdmvirtual_interface_vars: PLI_INT32 = 2407;
pub const uhdmvoid_typespec: PLI_INT32 = 2408;
pub const uhdmvpiArguments: PLI_INT32 = 2409;
pub const uhdmvpiClockingEvent: PLI_INT32 = 2410;
pub const uhdmvpiCondition: PLI_INT32 = 2411;
pub const uhdmvpiConditions: PLI_INT32 = 2412;
pub const uhdmvpiDisableCondition: PLI_INT32 = 2413;
pub const uhdmvpiElseStmt: PLI_INT32 = 2414;
pub const uhdmvpiExpr: PLI_INT32 = 2415;
pub const uhdmvpiExprs: PLI_INT32 = 2416;
pub const uhdmvpiForIncStmt: PLI_INT32 = 2417;
pub const uhdmvpiForIncStmts: PLI_INT32 = 2418;
pub const uhdmvpiForInitStmt: PLI_INT32 = 2419;
pub const uhdmvpiForInitStmts: PLI_INT32 = 2420;
pub const uhdmvpiIndex: PLI_INT32 = 2421;
pub const uhdmvpiInstance: PLI_INT32 = 2422;
pub const uhdmvpiLoopVars: PLI_INT32 = 2423;
pub const uhdmvpiProperty: PLI_INT32 = 2424;
pub const uhdmvpiPropertyExpr: PLI_INT32 = 2425;
pub const uhdmvpiSequenceExpr: PLI_INT32 = 2426;
pub const uhdmvpiStmt: PLI_INT32 = 2427;
pub const uhdmvpiUses: PLI_INT32 = 2428;
pub const uhdmwait_fork: PLI_INT32 = 2429;
pub const uhdmwait_stmt: PLI_INT32 = 2430;
pub const uhdmwaits: PLI_INT32 = 2431;
pub const uhdmweight: PLI_INT32 = 2432;
pub const uhdmwhile_stmt: PLI_INT32 = 2433;
pub const uhdmwidth_expr: PLI_INT32 = 2434;
pub const uhdmwith: PLI_INT32 = 2435;

// --- SV VPI Object Type (sv_vpi_user.h) --------------------------------------

/****************************** OBJECT TYPES ******************************/
pub const vpiPackage: PLI_INT32 = 600;
pub const vpiInterface: PLI_INT32 = 601;
pub const vpiProgram: PLI_INT32 = 602;
pub const vpiInterfaceArray: PLI_INT32 = 603;
pub const vpiProgramArray: PLI_INT32 = 604;
pub const vpiTypespec: PLI_INT32 = 605;
pub const vpiModport: PLI_INT32 = 606;
pub const vpiInterfaceTfDecl: PLI_INT32 = 607;
pub const vpiRefObj: PLI_INT32 = 608;
pub const vpiTypeParameter: PLI_INT32 = 609;

/* variables */
/* see uhdm.h pub const vpiVarBit: PLI_INT32 = ;vpiRegBit */
pub const vpiLongIntVar: PLI_INT32 = 610;
pub const vpiShortIntVar: PLI_INT32 = 611;
pub const vpiIntVar: PLI_INT32 = 612;
pub const vpiShortRealVar: PLI_INT32 = 613;
pub const vpiByteVar: PLI_INT32 = 614;
pub const vpiClassVar: PLI_INT32 = 615;
pub const vpiStringVar: PLI_INT32 = 616;
pub const vpiEnumVar: PLI_INT32 = 617;
pub const vpiStructVar: PLI_INT32 = 618;
pub const vpiUnionVar: PLI_INT32 = 619;
pub const vpiBitVar: PLI_INT32 = 620;
/* see uhdm.h pub const vpiLogicVar: PLI_INT32 = ;vpiReg */
/* see uhdm.h pub const vpiArrayVar: PLI_INT32 = ;vpiRegArray */
pub const vpiClassObj: PLI_INT32 = 621;
pub const vpiChandleVar: PLI_INT32 = 622;
pub const vpiPackedArrayVar: PLI_INT32 = 623;
pub const vpiVirtualInterfaceVar: PLI_INT32 = 728;

/* typespecs */
pub const vpiLongIntTypespec: PLI_INT32 = 625;
pub const vpiShortRealTypespec: PLI_INT32 = 626;
pub const vpiByteTypespec: PLI_INT32 = 627;
pub const vpiShortIntTypespec: PLI_INT32 = 628;
pub const vpiIntTypespec: PLI_INT32 = 629;
pub const vpiClassTypespec: PLI_INT32 = 630;
pub const vpiStringTypespec: PLI_INT32 = 631;
pub const vpiChandleTypespec: PLI_INT32 = 632;
pub const vpiEnumTypespec: PLI_INT32 = 633;
pub const vpiEnumConst: PLI_INT32 = 634;
pub const vpiIntegerTypespec: PLI_INT32 = 635;
pub const vpiTimeTypespec: PLI_INT32 = 636;
pub const vpiRealTypespec: PLI_INT32 = 637;
pub const vpiStructTypespec: PLI_INT32 = 638;
pub const vpiUnionTypespec: PLI_INT32 = 639;
pub const vpiBitTypespec: PLI_INT32 = 640;
pub const vpiLogicTypespec: PLI_INT32 = 641;
pub const vpiArrayTypespec: PLI_INT32 = 642;
pub const vpiVoidTypespec: PLI_INT32 = 643;
pub const vpiTypespecMember: PLI_INT32 = 644;
pub const vpiPackedArrayTypespec: PLI_INT32 = 692;
pub const vpiSequenceTypespec: PLI_INT32 = 696;
pub const vpiPropertyTypespec: PLI_INT32 = 697;
pub const vpiEventTypespec: PLI_INT32 = 698;
pub const vpiModuleTypespec: PLI_INT32 = 768; /* !!! NOT Standard !!! */
pub const vpiRefTypespec: PLI_INT32 = 769; /* !!! NOT Standard !!! */
pub const vpiLabel: PLI_INT32 = 770;
pub const vpiEndLabel: PLI_INT32 = 771;

pub const vpiClockingBlock: PLI_INT32 = 650;
pub const vpiClockingIODecl: PLI_INT32 = 651;
pub const vpiClassDefn: PLI_INT32 = 652;
pub const vpiConstraint: PLI_INT32 = 653;
pub const vpiConstraintOrdering: PLI_INT32 = 654;

pub const vpiDistItem: PLI_INT32 = 645;
pub const vpiAliasStmt: PLI_INT32 = 646;
pub const vpiThread: PLI_INT32 = 647;
pub const vpiMethodFuncCall: PLI_INT32 = 648;
pub const vpiMethodTaskCall: PLI_INT32 = 649;

/* concurrent assertions */
pub const vpiAssert: PLI_INT32 = 686;
pub const vpiAssume: PLI_INT32 = 687;
pub const vpiCover: PLI_INT32 = 688;
pub const vpiRestrict: PLI_INT32 = 901;

pub const vpiDisableCondition: PLI_INT32 = 689;
pub const vpiClockingEvent: PLI_INT32 = 690;

/* property decl, spec */
pub const vpiPropertyDecl: PLI_INT32 = 655;
pub const vpiPropertySpec: PLI_INT32 = 656;
pub const vpiPropertyExpr: PLI_INT32 = 657;
pub const vpiMulticlockSequenceExpr: PLI_INT32 = 658;
pub const vpiClockedSeq: PLI_INT32 = 659;
pub const vpiClockedProp: PLI_INT32 = 902;
pub const vpiPropertyInst: PLI_INT32 = 660;
pub const vpiSequenceDecl: PLI_INT32 = 661;
pub const vpiCaseProperty: PLI_INT32 = 662; /* property case */
pub const vpiCasePropertyItem: PLI_INT32 = 905; /* property case item */
pub const vpiSequenceInst: PLI_INT32 = 664;
pub const vpiImmediateAssert: PLI_INT32 = 665;
pub const vpiImmediateAssume: PLI_INT32 = 694;
pub const vpiImmediateCover: PLI_INT32 = 695;
pub const vpiReturn: PLI_INT32 = 666;
/* pattern */
pub const vpiAnyPattern: PLI_INT32 = 667;
pub const vpiTaggedPattern: PLI_INT32 = 668;
pub const vpiStructPattern: PLI_INT32 = 669;
/* do .. while */
pub const vpiDoWhile: PLI_INT32 = 670;
/* waits */
pub const vpiOrderedWait: PLI_INT32 = 671;
pub const vpiWaitFork: PLI_INT32 = 672;
/* disables */
pub const vpiDisableFork: PLI_INT32 = 673;
pub const vpiExpectStmt: PLI_INT32 = 674;
pub const vpiForeachStmt: PLI_INT32 = 675;
pub const vpiReturnStmt: PLI_INT32 = 691;
pub const vpiFinal: PLI_INT32 = 676;
pub const vpiExtends: PLI_INT32 = 677;
pub const vpiDistribution: PLI_INT32 = 678;
pub const vpiSeqFormalDecl: PLI_INT32 = 679;
pub const vpiPropFormalDecl: PLI_INT32 = 699;
pub const vpiArrayNet: PLI_INT32 = vpiNetArray;
pub const vpiEnumNet: PLI_INT32 = 680;
pub const vpiIntegerNet: PLI_INT32 = 681;
pub const vpiLogicNet: PLI_INT32 = vpiNet;
pub const vpiTimeNet: PLI_INT32 = 682;
pub const vpiStructNet: PLI_INT32 = 683;
pub const vpiBreak: PLI_INT32 = 684;
pub const vpiContinue: PLI_INT32 = 685;
pub const vpiPackedArrayNet: PLI_INT32 = 693;
pub const vpiConstraintExpr: PLI_INT32 = 747;
pub const vpiElseConst: PLI_INT32 = 748;
pub const vpiImplication: PLI_INT32 = 749;
pub const vpiConstrIf: PLI_INT32 = 738;
pub const vpiConstrIfElse: PLI_INT32 = 739;
pub const vpiConstrForEach: PLI_INT32 = 736;
pub const vpiLetDecl: PLI_INT32 = 903;
pub const vpiLetExpr: PLI_INT32 = 904;

/******************************** METHODS *********************************/
/************* methods used to traverse 1 to 1 relationships **************/
pub const vpiActual: PLI_INT32 = 700;

pub const vpiTypedefAlias: PLI_INT32 = 701;

pub const vpiIndexTypespec: PLI_INT32 = 702;
pub const vpiBaseTypespec: PLI_INT32 = 703;
pub const vpiElemTypespec: PLI_INT32 = 704;

pub const vpiInputSkew: PLI_INT32 = 706;
pub const vpiOutputSkew: PLI_INT32 = 707;
pub const vpiGlobalClocking: PLI_INT32 = 708;
pub const vpiDefaultClocking: PLI_INT32 = 709;
pub const vpiDefaultDisableIff: PLI_INT32 = 710;

pub const vpiOrigin: PLI_INT32 = 713;
pub const vpiPrefix: PLI_INT32 = 714;
pub const vpiWith: PLI_INT32 = 715;

pub const vpiProperty: PLI_INT32 = 718;

pub const vpiValueRange: PLI_INT32 = 720;
pub const vpiPattern: PLI_INT32 = 721;
pub const vpiWeight: PLI_INT32 = 722;
pub const vpiConstraintItem: PLI_INT32 = 746;

/************ methods used to traverse 1 to many relationships ************/
pub const vpiTypedef: PLI_INT32 = 725;
pub const vpiImportTypespec: PLI_INT32 = 726;
pub const vpiDerivedClasses: PLI_INT32 = 727;
pub const vpiInterfaceDecl: PLI_INT32 = vpiVirtualInterfaceVar; /* interface decl deprecated */

pub const vpiMethods: PLI_INT32 = 730;
pub const vpiSolveBefore: PLI_INT32 = 731;
pub const vpiSolveAfter: PLI_INT32 = 732;

pub const vpiWaitingProcesses: PLI_INT32 = 734;

pub const vpiMessages: PLI_INT32 = 735;
pub const vpiLoopVars: PLI_INT32 = 737;

pub const vpiConcurrentAssertions: PLI_INT32 = 740;
pub const vpiMatchItem: PLI_INT32 = 741;
pub const vpiMember: PLI_INT32 = 742;
pub const vpiElement: PLI_INT32 = 743;

/************* methods used to traverse 1 to many relationships ***************/
pub const vpiAssertion: PLI_INT32 = 744;

/*********** methods used to traverse both 1-1 and 1-many relations ***********/
pub const vpiInstance: PLI_INT32 = 745;

/**************************************************************************/
/************************ generic object properties ***********************/
/**************************************************************************/

pub const vpiTop: PLI_INT32 = 600;

pub const vpiUnit: PLI_INT32 = 602;
pub const vpiJoinType: PLI_INT32 = 603;
pub const vpiJoin: PLI_INT32 = 0;
pub const vpiJoinNone: PLI_INT32 = 1;
pub const vpiJoinAny: PLI_INT32 = 2;
pub const vpiAccessType: PLI_INT32 = 604;
pub const vpiForkJoinAcc: PLI_INT32 = 1;
pub const vpiExternAcc: PLI_INT32 = 2;
pub const vpiDPIExportAcc: PLI_INT32 = 3;
pub const vpiDPIImportAcc: PLI_INT32 = 4;

pub const vpiArrayType: PLI_INT32 = 606;
pub const vpiStaticArray: PLI_INT32 = 1;
pub const vpiDynamicArray: PLI_INT32 = 2;
pub const vpiAssocArray: PLI_INT32 = 3;
pub const vpiQueueArray: PLI_INT32 = 4;
pub const vpiArrayMember: PLI_INT32 = 607;

pub const vpiIsRandomized: PLI_INT32 = 608;
pub const vpiLocalVarDecls: PLI_INT32 = 609;
pub const vpiOpStrong: PLI_INT32 = 656; /* strength of temporal operator */
pub const vpiRandType: PLI_INT32 = 610;
pub const vpiNotRand: PLI_INT32 = 1;
pub const vpiRand: PLI_INT32 = 2;
pub const vpiRandC: PLI_INT32 = 3;
pub const vpiPortType: PLI_INT32 = 611;
pub const vpiInterfacePort: PLI_INT32 = 1;
pub const vpiModportPort: PLI_INT32 = 2;
/* vpiPort is also a port type. It is defined in vpi_user.h */

pub const vpiConstantVariable: PLI_INT32 = 612;
pub const vpiStructUnionMember: PLI_INT32 = 615;

pub const vpiVisibility: PLI_INT32 = 620;
pub const vpiPublicVis: PLI_INT32 = 1;
pub const vpiProtectedVis: PLI_INT32 = 2;
pub const vpiLocalVis: PLI_INT32 = 3;

/* Return values for vpiConstType property */
pub const vpiOneStepConst: PLI_INT32 = 9;
pub const vpiUnboundedConst: PLI_INT32 = 10;
pub const vpiNullConst: PLI_INT32 = 11;

pub const vpiAlwaysType: PLI_INT32 = 624;
pub const vpiAlwaysComb: PLI_INT32 = 2;
pub const vpiAlwaysFF: PLI_INT32 = 3;
pub const vpiAlwaysLatch: PLI_INT32 = 4;

pub const vpiDistType: PLI_INT32 = 625;
pub const vpiEqualDist: PLI_INT32 = 1; /* constraint equal distribution */
pub const vpiDivDist: PLI_INT32 = 2; /* constraint divided distribution */

pub const vpiPacked: PLI_INT32 = 630;
pub const vpiTagged: PLI_INT32 = 632;
pub const vpiRef: PLI_INT32 = 6; /* Return value for vpiDirection property */
pub const vpiVirtual: PLI_INT32 = 635;
pub const vpiHasActual: PLI_INT32 = 636;
pub const vpiIsConstraintEnabled: PLI_INT32 = 638;
pub const vpiSoft: PLI_INT32 = 639;

pub const vpiClassType: PLI_INT32 = 640;
pub const vpiMailboxClass: PLI_INT32 = 1;
pub const vpiSemaphoreClass: PLI_INT32 = 2;
pub const vpiUserDefinedClass: PLI_INT32 = 3;
pub const vpiProcessClass: PLI_INT32 = 4;
pub const vpiMethod: PLI_INT32 = 645;
pub const vpiIsClockInferred: PLI_INT32 = 649;
pub const vpiIsDeferred: PLI_INT32 = 657;
pub const vpiIsFinal: PLI_INT32 = 658;
pub const vpiIsCoverSequence: PLI_INT32 = 659;
pub const vpiQualifier: PLI_INT32 = 650;
pub const vpiNoQualifier: PLI_INT32 = 0;
pub const vpiUniqueQualifier: PLI_INT32 = 1;
pub const vpiPriorityQualifier: PLI_INT32 = 2;
pub const vpiTaggedQualifier: PLI_INT32 = 4;
pub const vpiRandQualifier: PLI_INT32 = 8;
pub const vpiInsideQualifier: PLI_INT32 = 16;

pub const vpiInputEdge: PLI_INT32 = 651; /* returns vpiNoEdge, vpiPosedge,
                                         vpiNegedge */
pub const vpiOutputEdge: PLI_INT32 = 652; /* returns vpiNoEdge, vpiPosedge,
                                          vpiNegedge */
pub const vpiGeneric: PLI_INT32 = 653;

/* Compatibility-mode property and values (object argument == NULL) */
pub const vpiCompatibilityMode: PLI_INT32 = 654;
pub const vpiMode1364v1995: PLI_INT32 = 1;
pub const vpiMode1364v2001: PLI_INT32 = 2;
pub const vpiMode1364v2005: PLI_INT32 = 3;
pub const vpiMode1800v2005: PLI_INT32 = 4;
pub const vpiMode1800v2009: PLI_INT32 = 5;

pub const vpiPackedArrayMember: PLI_INT32 = 655;
pub const vpiStartLine: PLI_INT32 = 661;
pub const vpiColumn: PLI_INT32 = 662;
pub const vpiEndLine: PLI_INT32 = 663;
pub const vpiEndColumn: PLI_INT32 = 664;

/* memory allocation scheme for transient objects */
pub const vpiAllocScheme: PLI_INT32 = 658;
pub const vpiAutomaticScheme: PLI_INT32 = 1;
pub const vpiDynamicScheme: PLI_INT32 = 2;
pub const vpiOtherScheme: PLI_INT32 = 3;

pub const vpiObjId: PLI_INT32 = 660;

pub const vpiDPIPure: PLI_INT32 = 665;
pub const vpiDPIContext: PLI_INT32 = 666;
pub const vpiDPICStr: PLI_INT32 = 667;
pub const vpiDPI: PLI_INT32 = 1;
pub const vpiDPIC: PLI_INT32 = 2;
pub const vpiDPICIdentifier: PLI_INT32 = 668;

/******************************** Operators *******************************/
pub const vpiImplyOp: PLI_INT32 = 50; /* -> implication operator */
pub const vpiNonOverlapImplyOp: PLI_INT32 = 51; /* |=> nonoverlapped implication */
pub const vpiOverlapImplyOp: PLI_INT32 = 52; /* |-> overlapped implication operator */
pub const vpiAcceptOnOp: PLI_INT32 = 83; /* accept_on operator */
pub const vpiRejectOnOp: PLI_INT32 = 84; /* reject_on operator */
pub const vpiSyncAcceptOnOp: PLI_INT32 = 85; /* sync_accept_on operator */
pub const vpiSyncRejectOnOp: PLI_INT32 = 86; /* sync_reject_on operator */
pub const vpiOverlapFollowedByOp: PLI_INT32 = 87; /* overlapped followed_by operator */
pub const vpiNonOverlapFollowedByOp: PLI_INT32 = 88; /* nonoverlapped followed_by operator */
pub const vpiNexttimeOp: PLI_INT32 = 89; /* nexttime operator */
pub const vpiAlwaysOp: PLI_INT32 = 90; /* always operator */
pub const vpiEventuallyOp: PLI_INT32 = 91; /* eventually operator */
pub const vpiUntilOp: PLI_INT32 = 92; /* until operator */
pub const vpiUntilWithOp: PLI_INT32 = 93; /* until_with operator */

pub const vpiUnaryCycleDelayOp: PLI_INT32 = 53; /* binary cycle delay (##) operator */
pub const vpiCycleDelayOp: PLI_INT32 = 54; /* binary cycle delay (##) operator */
pub const vpiIntersectOp: PLI_INT32 = 55; /* intersection operator */
pub const vpiFirstMatchOp: PLI_INT32 = 56; /* first_match operator */
pub const vpiThroughoutOp: PLI_INT32 = 57; /* throughout operator */
pub const vpiWithinOp: PLI_INT32 = 58; /* within operator */
pub const vpiRepeatOp: PLI_INT32 = 59; /* [=] nonconsecutive repetition */
pub const vpiConsecutiveRepeatOp: PLI_INT32 = 60; /* [*] consecutive repetition */
pub const vpiGotoRepeatOp: PLI_INT32 = 61; /* [->] goto repetition */

pub const vpiPostIncOp: PLI_INT32 = 62; /* ++ post-increment */
pub const vpiPreIncOp: PLI_INT32 = 63; /* ++ pre-increment */
pub const vpiPostDecOp: PLI_INT32 = 64; /* -- post-decrement */
pub const vpiPreDecOp: PLI_INT32 = 65; /* -- pre-decrement */

pub const vpiMatchOp: PLI_INT32 = 66; /* match() operator */
pub const vpiCastOp: PLI_INT32 = 67; /* type'() operator */
pub const vpiIffOp: PLI_INT32 = 68; /* iff operator */
pub const vpiWildEqOp: PLI_INT32 = 69; /* ==? operator */
pub const vpiWildNeqOp: PLI_INT32 = 70; /* !=? operator */

pub const vpiStreamLROp: PLI_INT32 = 71; /* left-to-right streaming {>>} operator */
pub const vpiStreamRLOp: PLI_INT32 = 72; /* right-to-left streaming {<<} operator */

pub const vpiMatchedOp: PLI_INT32 = 73; /* the .matched sequence operation */
pub const vpiTriggeredOp: PLI_INT32 = 74; /* the .triggered sequence operation */
pub const vpiAssignmentPatternOp: PLI_INT32 = 75; /* '{} assignment pattern */
pub const vpiMultiAssignmentPatternOp: PLI_INT32 = 76; /* '{n{}} multi assignment pattern */
pub const vpiIfOp: PLI_INT32 = 77; /* if operator */
pub const vpiIfElseOp: PLI_INT32 = 78; /* if-else operator */
pub const vpiCompAndOp: PLI_INT32 = 79; /* Composite and operator */
pub const vpiCompOrOp: PLI_INT32 = 80; /* Composite or operator */
pub const vpiImpliesOp: PLI_INT32 = 94; /* implies operator */
pub const vpiInsideOp: PLI_INT32 = 95; /* inside operator */
pub const vpiTypeOp: PLI_INT32 = 81; /* type operator */
pub const vpiAssignmentOp: PLI_INT32 = 82; /* Normal assignment */

/*********************** task/function properties ***********************/
pub const vpiOtherFunc: PLI_INT32 = 6; /* returns other types; for property vpiFuncType */
/* vpiValid,vpiValidTrue,vpiValidFalse are deprecated in 1800-2009 */

/*********************** value for vpiValid *****************************/
pub const vpiValidUnknown: PLI_INT32 = 2; /* Validity of variable is unknown */

/************************** STRUCTURE DEFINITIONS *************************/

/***************************** structure *****************************/

/**************************** CALLBACK REASONS ****************************/
pub const cbStartOfThread: PLI_INT32 = 600; /* callback on thread creation */
pub const cbEndOfThread: PLI_INT32 = 601; /* callback on thread termination */
pub const cbEnterThread: PLI_INT32 = 602; /* callback on reentering thread */
pub const cbStartOfFrame: PLI_INT32 = 603; /* callback on frame creation */
pub const cbEndOfFrame: PLI_INT32 = 604; /* callback on frame exit */
pub const cbSizeChange: PLI_INT32 = 605; /* callback on array variable size change */
pub const cbCreateObj: PLI_INT32 = 700; /* callback on class object creation */
pub const cbReclaimObj: PLI_INT32 = 701; /* callback on class object reclaimed by automatic memory management */

pub const cbEndOfObject: PLI_INT32 = 702; /* callback on transient object deletion */

/************************* FUNCTION DECLARATIONS **************************/

/**************************************************************************/
/*************************** Coverage VPI *********************************/
/**************************************************************************/

/* coverage control */
pub const vpiCoverageStart: PLI_INT32 = 750;
pub const vpiCoverageStOp: PLI_INT32 = 751;
pub const vpiCoverageReset: PLI_INT32 = 752;
pub const vpiCoverageCheck: PLI_INT32 = 753;
pub const vpiCoverageMerge: PLI_INT32 = 754;
pub const vpiCoverageSave: PLI_INT32 = 755;

/* coverage type properties */
pub const vpiAssertCoverage: PLI_INT32 = 760;
pub const vpiFsmStateCoverage: PLI_INT32 = 761;
pub const vpiStatementCoverage: PLI_INT32 = 762;
pub const vpiToggleCoverage: PLI_INT32 = 763;

/* coverage status properties */
pub const vpiCovered: PLI_INT32 = 765;
pub const vpiCoverMax: PLI_INT32 = 766;
pub const vpiCoveredCount: PLI_INT32 = 767;

/* assertion-specific coverage status properties */
pub const vpiAssertAttemptCovered: PLI_INT32 = 770;
pub const vpiAssertSuccessCovered: PLI_INT32 = 771;
pub const vpiAssertFailureCovered: PLI_INT32 = 772;
pub const vpiAssertVacuousSuccessCovered: PLI_INT32 = 773;
pub const vpiAssertDisableCovered: PLI_INT32 = 774;
pub const vpiAssertKillCovered: PLI_INT32 = 777;

/* FSM-specific coverage status properties */
pub const vpiFsmStates: PLI_INT32 = 775;
pub const vpiFsmStateExpression: PLI_INT32 = 776;

/* FSM handle types */
pub const vpiFsm: PLI_INT32 = 758;
pub const vpiFsmHandle: PLI_INT32 = 759;

/***************************************************************************/
/***************************** Assertion VPI *******************************/
/***************************************************************************/

/* assertion callback types */
pub const cbAssertionStart: PLI_INT32 = 606;
pub const cbAssertionSuccess: PLI_INT32 = 607;
pub const cbAssertionFailure: PLI_INT32 = 608;
pub const cbAssertionVacuousSuccess: PLI_INT32 = 657;
pub const cbAssertionDisabledEvaluation: PLI_INT32 = 658;
pub const cbAssertionStepSuccess: PLI_INT32 = 609;
pub const cbAssertionStepFailure: PLI_INT32 = 610;
pub const cbAssertionLock: PLI_INT32 = 661;
pub const cbAssertionUnlock: PLI_INT32 = 662;
pub const cbAssertionDisable: PLI_INT32 = 611;
pub const cbAssertionEnable: PLI_INT32 = 612;
pub const cbAssertionReset: PLI_INT32 = 613;
pub const cbAssertionKill: PLI_INT32 = 614;
pub const cbAssertionEnablePassAction: PLI_INT32 = 645;
pub const cbAssertionEnableFailAction: PLI_INT32 = 646;
pub const cbAssertionDisablePassAction: PLI_INT32 = 647;
pub const cbAssertionDisableFailAction: PLI_INT32 = 648;
pub const cbAssertionEnableNonvacuousAction: PLI_INT32 = 649;
pub const cbAssertionDisableVacuousAction: PLI_INT32 = 650;

/* assertion "system" callback types */
pub const cbAssertionSysInitialized: PLI_INT32 = 615;
pub const cbAssertionSysOn: PLI_INT32 = 616;
pub const cbAssertionSysOff: PLI_INT32 = 617;
pub const cbAssertionSysKill: PLI_INT32 = 631;
pub const cbAssertionSysLock: PLI_INT32 = 659;
pub const cbAssertionSysUnlock: PLI_INT32 = 660;
pub const cbAssertionSysEnd: PLI_INT32 = 618;
pub const cbAssertionSysReset: PLI_INT32 = 619;
pub const cbAssertionSysEnablePassAction: PLI_INT32 = 651;
pub const cbAssertionSysEnableFailAction: PLI_INT32 = 652;
pub const cbAssertionSysDisablePassAction: PLI_INT32 = 653;
pub const cbAssertionSysDisableFailAction: PLI_INT32 = 654;
pub const cbAssertionSysEnableNonvacuousAction: PLI_INT32 = 655;
pub const cbAssertionSysDisableVacuousAction: PLI_INT32 = 656;

/* assertion control constants */
pub const vpiAssertionLock: PLI_INT32 = 645;
pub const vpiAssertionUnlock: PLI_INT32 = 646;
pub const vpiAssertionDisable: PLI_INT32 = 620;
pub const vpiAssertionEnable: PLI_INT32 = 621;
pub const vpiAssertionReset: PLI_INT32 = 622;
pub const vpiAssertionKill: PLI_INT32 = 623;
pub const vpiAssertionEnableStep: PLI_INT32 = 624;
pub const vpiAssertionDisableStep: PLI_INT32 = 625;
pub const vpiAssertionClockSteps: PLI_INT32 = 626;
pub const vpiAssertionSysLock: PLI_INT32 = 647;
pub const vpiAssertionSysUnlock: PLI_INT32 = 648;
pub const vpiAssertionSysOn: PLI_INT32 = 627;
pub const vpiAssertionSysOff: PLI_INT32 = 628;
pub const vpiAssertionSysKill: PLI_INT32 = 632;
pub const vpiAssertionSysEnd: PLI_INT32 = 629;
pub const vpiAssertionSysReset: PLI_INT32 = 630;
pub const vpiAssertionDisablePassAction: PLI_INT32 = 633;
pub const vpiAssertionEnablePassAction: PLI_INT32 = 634;
pub const vpiAssertionDisableFailAction: PLI_INT32 = 635;
pub const vpiAssertionEnableFailAction: PLI_INT32 = 636;
pub const vpiAssertionDisableVacuousAction: PLI_INT32 = 637;
pub const vpiAssertionEnableNonvacuousAction: PLI_INT32 = 638;
pub const vpiAssertionSysEnablePassAction: PLI_INT32 = 639;
pub const vpiAssertionSysEnableFailAction: PLI_INT32 = 640;
pub const vpiAssertionSysDisablePassAction: PLI_INT32 = 641;
pub const vpiAssertionSysDisableFailAction: PLI_INT32 = 642;
pub const vpiAssertionSysEnableNonvacuousAction: PLI_INT32 = 643;
pub const vpiAssertionSysDisableVacuousAction: PLI_INT32 = 644;

// ── LSP-internal synthetic type IDs ──────────────────────────────────────────
//
// These are NOT real VPI or UHDM object types.  They live above the highest
// UHDM type value (2435) and are used exclusively between `tokens.rs` and
// `semantic_tokens.rs` to convey token categories that VPI encodes as
// properties rather than distinct object types.

/// Synthetic: port whose `vpiDirection == vpiInput`.
pub const TOKEN_PORT_INPUT: PLI_INT32 = 10_001;
/// Synthetic: port whose `vpiDirection == vpiOutput`.
pub const TOKEN_PORT_OUTPUT: PLI_INT32 = 10_002;
/// Synthetic: port whose `vpiDirection == vpiInout`.
pub const TOKEN_PORT_INOUT: PLI_INT32 = 10_003;
/// Synthetic: name declared by a `typedef` statement (parse-tree origin).
pub const TOKEN_TYPEDEF_NAME: PLI_INT32 = 10_004;
/// Synthetic: named PORT-connection label (the `.clk` of `.clk(wa)`),
/// identified structurally by the parse-tree classifier.  Encoded as the
/// label's usual token type carrying the `connectionLabel` modifier.
pub const TOKEN_PORT_CONN_LABEL: PLI_INT32 = 10_005;
/// Synthetic: named PARAMETER-override label (the `.W` of `m #(.W(4))`),
/// identified structurally by the parse-tree classifier.  Encoded as the
/// override label's usual token type carrying the `connectionLabel` modifier.
pub const TOKEN_PARAM_CONN_LABEL: PLI_INT32 = 10_006;

// ── Structures ────────────────────────────────────────────────────────────────

/// `s_vpi_time` — VPI time value.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VpiTime {
    /// Time type: `vpiScaledRealTime`, `vpiSimTime`, or `vpiSuppressTime`.
    pub type_: PLI_INT32,
    /// High 32 bits (used for `vpiSimTime`).
    pub high: PLI_UINT32,
    /// Low 32 bits (used for `vpiSimTime`).
    pub low: PLI_UINT32,
    /// Real value (used for `vpiScaledRealTime`).
    pub real: c_double,
}

/// `s_vpi_delay` — delay specification.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VpiDelay {
    /// Pointer to array of delay values (application-allocated).
    pub da: *mut VpiTime,
    pub no_of_delays: PLI_INT32,
    pub time_type: PLI_INT32,
    pub mtm_flag: PLI_INT32,
    pub append_flag: PLI_INT32,
    pub pulsere_flag: PLI_INT32,
}

/// `s_vpi_vecval` — single element of a 4-state vector.
/// Encoding: `ab` = `00`→0, `10`→1, `11`→X, `01`→Z.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VpiVecval {
    pub aval: PLI_UINT32,
    pub bval: PLI_UINT32,
}

/// `s_vpi_strengthval` — strength of a scalar net.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VpiStrengthval {
    pub logic: PLI_INT32,
    pub s0: PLI_INT32,
    pub s1: PLI_INT32,
}

/// Inner union of `VpiValue`.
#[repr(C)]
pub union VpiValueData {
    pub str_: *mut PLI_BYTE8,
    pub scalar: PLI_INT32,
    pub integer: PLI_INT64,
    pub uint: PLI_UINT64,
    pub real: c_double,
    pub time: *mut VpiTime,
    pub vector: *mut VpiVecval,
    pub strength: *mut VpiStrengthval,
    pub misc: *mut PLI_BYTE8,
}

/// `s_vpi_value` — generic value container used by `vpi_get_value` / `vpi_put_value`.
#[repr(C)]
pub struct VpiValue {
    /// Format selector: one of the `vpi*Val` constants.
    pub format: PLI_INT32,
    pub value: VpiValueData,
}

/// Inner union of `VpiArrayValue`.
#[repr(C)]
pub union VpiArrayValueData {
    pub integers: *mut PLI_INT32,
    pub shortints: *mut PLI_INT16,
    pub longints: *mut PLI_INT64,
    pub rawvals: *mut PLI_BYTE8,
    pub vectors: *mut VpiVecval,
    pub times: *mut VpiTime,
    pub reals: *mut c_double,
    pub shortreals: *mut c_float,
}

/// `s_vpi_arrayvalue` — array value for `vpi_get_value_array` / `vpi_put_value_array`.
#[repr(C)]
pub struct VpiArrayValue {
    pub format: PLI_UINT32,
    pub flags: PLI_UINT32,
    pub value: VpiArrayValueData,
}

/// `s_vpi_systf_data` — registration data for user-defined system tasks/functions.
#[repr(C)]
pub struct VpiSystfData {
    /// `vpiSysTask` or `vpiSysFunc`.
    pub type_: PLI_INT32,
    pub sysfunctype: PLI_INT32,
    /// Task/function name; first character must be `$`.
    pub tfname: *mut PLI_BYTE8,
    pub calltf: Option<unsafe extern "C" fn(*mut PLI_BYTE8) -> PLI_INT32>,
    pub compiletf: Option<unsafe extern "C" fn(*mut PLI_BYTE8) -> PLI_INT32>,
    pub sizetf: Option<unsafe extern "C" fn(*mut PLI_BYTE8) -> PLI_INT32>,
    pub user_data: *mut PLI_BYTE8,
}

/// `s_vpi_vlog_info` — simulator version information.
#[repr(C)]
pub struct VpiVlogInfo {
    pub argc: PLI_INT32,
    pub argv: *mut *mut PLI_BYTE8,
    pub product: *mut PLI_BYTE8,
    pub version: *mut PLI_BYTE8,
}

/// `s_vpi_error_info` — error/diagnostic information.
#[repr(C)]
pub struct VpiErrorInfo {
    /// State when error occurred: `vpiCompile`, `vpiPLI`, or `vpiRun`.
    pub state: PLI_INT32,
    /// Severity: `vpiNotice` .. `vpiInternal`.
    pub level: PLI_INT32,
    pub message: *mut PLI_BYTE8,
    pub product: *mut PLI_BYTE8,
    pub code: *mut PLI_BYTE8,
    pub file: *mut PLI_BYTE8,
    pub line: PLI_INT32,
}

/// `s_cb_data` — callback registration data.
#[repr(C)]
pub struct CbData {
    pub reason: PLI_INT32,
    pub cb_rtn: Option<unsafe extern "C" fn(*mut CbData) -> PLI_INT32>,
    pub obj: VpiHandle,
    pub time: *mut VpiTime,
    pub value: *mut VpiValue,
    pub index: PLI_INT32,
    pub user_data: *mut PLI_BYTE8,
}

// ── VPI function declarations ─────────────────────────────────────────────────
// Symbols are provided by the `uhdm` static library (linked via build.rs).
//
// The `#[link]` attributes duplicate the static archives that build.rs emits
// through `cargo:rustc-link-lib`.  With a lib target in the package, cargo
// applies that build-script output only to the lib; binary targets that pull
// this module in via `#[path]` (and therefore never reference the `llg` rlib)
// would otherwise miss the archives and fail to link.

#[link(name = "surelog", kind = "static")]
#[link(name = "uhdm", kind = "static")]
#[link(name = "antlr4-runtime", kind = "static")]
#[link(name = "capnp", kind = "static")]
#[link(name = "kj-async", kind = "static")]
#[link(name = "kj", kind = "static")]
#[link(name = "stdc++")]
#[link(name = "z")]
#[link(name = "pthread")]
unsafe extern "C" {
    // ── Callback ──────────────────────────────────────────────────────────────

    pub fn vpi_register_cb(cb_data_p: *mut CbData) -> VpiHandle;
    pub fn vpi_remove_cb(cb_obj: VpiHandle) -> PLI_INT32;
    pub fn vpi_get_cb_info(object: VpiHandle, cb_data_p: *mut CbData);
    pub fn vpi_register_systf(systf_data_p: *mut VpiSystfData) -> VpiHandle;
    pub fn vpi_get_systf_info(object: VpiHandle, systf_data_p: *mut VpiSystfData);

    // ── Handle retrieval ──────────────────────────────────────────────────────

    pub fn vpi_handle_by_name(name: *mut PLI_BYTE8, scope: VpiHandle) -> VpiHandle;
    pub fn vpi_handle_by_index(object: VpiHandle, indx: PLI_INT32) -> VpiHandle;

    // ── Traversal ─────────────────────────────────────────────────────────────

    pub fn vpi_handle(type_: PLI_INT32, ref_handle: VpiHandle) -> VpiHandle;
    pub fn vpi_iterate(type_: PLI_INT32, ref_handle: VpiHandle) -> VpiHandle;
    pub fn vpi_scan(iterator: VpiHandle) -> VpiHandle;

    // ── Properties ────────────────────────────────────────────────────────────

    pub fn vpi_get(property: PLI_INT32, object: VpiHandle) -> PLI_INT32;
    pub fn vpi_get64(property: PLI_INT32, object: VpiHandle) -> PLI_INT64;
    /// Returns a pointer into VPI-internal storage valid until the next call.
    /// Do **not** free this pointer.
    pub fn vpi_get_str(property: PLI_INT32, object: VpiHandle) -> *mut PLI_BYTE8;

    // ── Delay ─────────────────────────────────────────────────────────────────

    pub fn vpi_get_delays(object: VpiHandle, delay_p: *mut VpiDelay);
    pub fn vpi_put_delays(object: VpiHandle, delay_p: *mut VpiDelay);

    // ── Value ─────────────────────────────────────────────────────────────────

    pub fn vpi_get_value(expr: VpiHandle, value_p: *mut VpiValue);
    pub fn vpi_put_value(
        object: VpiHandle,
        value_p: *mut VpiValue,
        time_p: *mut VpiTime,
        flags: PLI_INT32,
    ) -> VpiHandle;
    pub fn vpi_get_value_array(
        object: VpiHandle,
        arrayvalue_p: *mut VpiArrayValue,
        index_p: *mut PLI_INT32,
        num: PLI_UINT32,
    );
    pub fn vpi_put_value_array(
        object: VpiHandle,
        arrayvalue_p: *mut VpiArrayValue,
        index_p: *mut PLI_INT32,
        num: PLI_UINT32,
    );

    // ── Time ──────────────────────────────────────────────────────────────────

    pub fn vpi_get_time(object: VpiHandle, time_p: *mut VpiTime);

    // ── I/O ───────────────────────────────────────────────────────────────────

    pub fn vpi_mcd_open(file_name: *mut PLI_BYTE8) -> PLI_UINT32;
    pub fn vpi_mcd_close(mcd: PLI_UINT32) -> PLI_UINT32;
    pub fn vpi_mcd_name(cd: PLI_UINT32) -> *mut PLI_BYTE8;
    pub fn vpi_mcd_printf(mcd: PLI_UINT32, format: *const PLI_BYTE8, ...) -> PLI_INT32;
    pub fn vpi_printf(format: *const PLI_BYTE8, ...) -> PLI_INT32;

    // ── Utility ───────────────────────────────────────────────────────────────

    pub fn vpi_compare_objects(object1: VpiHandle, object2: VpiHandle) -> PLI_INT32;
    pub fn vpi_chk_error(error_info_p: *mut VpiErrorInfo) -> PLI_INT32;
    /// Deprecated in IEEE 1800-2009; prefer `vpi_release_handle`.
    pub fn vpi_free_object(object: VpiHandle) -> PLI_INT32;
    pub fn vpi_release_handle(object: VpiHandle) -> PLI_INT32;
    pub fn vpi_get_vlog_info(vlog_info_p: *mut VpiVlogInfo) -> PLI_INT32;

    // ── 1364-2001 additions ───────────────────────────────────────────────────

    pub fn vpi_get_data(
        id: PLI_INT32,
        data_loc: *mut PLI_BYTE8,
        num_of_bytes: PLI_INT32,
    ) -> PLI_INT32;
    pub fn vpi_put_data(
        id: PLI_INT32,
        data_loc: *mut PLI_BYTE8,
        num_of_bytes: PLI_INT32,
    ) -> PLI_INT32;
    pub fn vpi_get_userdata(obj: VpiHandle) -> *mut c_void;
    pub fn vpi_put_userdata(obj: VpiHandle, userdata: *mut c_void) -> PLI_INT32;
    pub fn vpi_flush() -> PLI_INT32;
    pub fn vpi_mcd_flush(mcd: PLI_UINT32) -> PLI_INT32;
    pub fn vpi_control(operation: PLI_INT32, ...) -> PLI_INT32;
    pub fn vpi_handle_by_multi_index(
        obj: VpiHandle,
        num_index: PLI_INT32,
        index_array: *mut PLI_INT32,
    ) -> VpiHandle;
    pub fn vpi_handle_multi(
        type_: PLI_INT32,
        ref_handle1: VpiHandle,
        ref_handle2: VpiHandle,
        ...
    ) -> VpiHandle;
}

// ── Safe wrappers ─────────────────────────────────────────────────────────────

/// An owned VPI handle that calls `vpi_release_handle` on drop.
///
/// Handles obtained from `vpi_handle_by_name`, `vpi_handle` (1-to-1 nav),
/// `vpi_register_cb`, and partial `VpiIter` drops are owning handles and
/// should be wrapped with this type.
pub struct OwnedHandle(VpiHandle);

impl OwnedHandle {
    /// Wraps a raw handle. Returns `None` if the pointer is null.
    pub fn new(raw: VpiHandle) -> Option<Self> {
        if raw.is_null() {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// Returns the raw handle without releasing ownership.
    pub fn raw(&self) -> VpiHandle {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                vpi_release_handle(self.0);
            }
        }
    }
}

/// An iterator over a 1-to-many VPI relationship (from `vpi_iterate`).
///
/// Yields raw `VpiHandle` values. The underlying iterator handle is released
/// automatically if the iterator is dropped before exhaustion.
pub struct VpiIter {
    iter_handle: VpiHandle,
    done: bool,
}

impl VpiIter {
    fn new(iter_handle: VpiHandle) -> Self {
        Self {
            iter_handle,
            done: false,
        }
    }
}

impl Drop for VpiIter {
    fn drop(&mut self) {
        // vpi_scan frees the iterator when it returns null; if not done yet
        // we must free it ourselves.
        if !self.done && !self.iter_handle.is_null() {
            unsafe {
                vpi_release_handle(self.iter_handle);
            }
        }
    }
}

impl Iterator for VpiIter {
    /// Raw handles from `vpi_scan` are borrowed from the design model and
    /// must not be freed by the caller.
    type Item = VpiHandle;

    fn next(&mut self) -> Option<VpiHandle> {
        if self.done || self.iter_handle.is_null() {
            return None;
        }
        let h = unsafe { vpi_scan(self.iter_handle) };
        if h.is_null() {
            // vpi_scan freed the iterator; mark done so Drop skips it.
            self.done = true;
            None
        } else {
            Some(h)
        }
    }
}

// ── Traversal ─────────────────────────────────────────────────────────────────

/// Returns a handle for a 1-to-1 relationship (e.g. `vpi_handle(vpiScope, h)`).
/// Returns `None` if no such object exists.
pub fn handle(type_: PLI_INT32, ref_handle: VpiHandle) -> Option<OwnedHandle> {
    OwnedHandle::new(unsafe { vpi_handle(type_, ref_handle) })
}

/// Returns an iterator over a 1-to-many relationship.
/// Returns `None` if the object has no children of that type.
pub fn iterate(type_: PLI_INT32, ref_handle: VpiHandle) -> Option<VpiIter> {
    let h = unsafe { vpi_iterate(type_, ref_handle) };
    if h.is_null() {
        None
    } else {
        Some(VpiIter::new(h))
    }
}

/// Returns a handle to an object by hierarchical name.
/// `scope` may be null to search the global scope.
pub fn handle_by_name(name: &str, scope: VpiHandle) -> Option<OwnedHandle> {
    let cname = CString::new(name).ok()?;
    OwnedHandle::new(unsafe { vpi_handle_by_name(cname.as_ptr() as *mut _, scope) })
}

/// Returns a handle to an array element by index.
pub fn handle_by_index(object: VpiHandle, indx: PLI_INT32) -> Option<OwnedHandle> {
    OwnedHandle::new(unsafe { vpi_handle_by_index(object, indx) })
}

/// Returns `true` if two handles refer to the same VPI object.
pub fn compare_objects(obj1: VpiHandle, obj2: VpiHandle) -> bool {
    unsafe { vpi_compare_objects(obj1, obj2) != 0 }
}

// ── Properties ────────────────────────────────────────────────────────────────

/// Returns an integer property (e.g. `vpi_get(vpiType, h)`).
pub fn get(property: PLI_INT32, object: VpiHandle) -> PLI_INT32 {
    unsafe { vpi_get(property, object) }
}

/// Returns a 64-bit integer property.
pub fn get64(property: PLI_INT32, object: VpiHandle) -> PLI_INT64 {
    unsafe { vpi_get64(property, object) }
}

/// Returns a string property copied into an owned `String`.
///
/// The raw pointer from `vpi_get_str` is valid only until the next call to
/// `vpi_get_str`, so it is always copied here.
pub fn get_str(property: PLI_INT32, object: VpiHandle) -> String {
    let ptr = unsafe { vpi_get_str(property, object) };
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

// ── Convenience property accessors ───────────────────────────────────────────

/// Returns the `vpiType` of an object as an integer constant (e.g. `vpiModule`).
pub fn obj_type(object: VpiHandle) -> PLI_INT32 {
    get(vpiType, object)
}

/// Returns the `vpiName` of an object.
pub fn obj_name(object: VpiHandle) -> String {
    get_str(vpiName, object)
}

/// Returns the `vpiFullName` of an object.
pub fn obj_full_name(object: VpiHandle) -> String {
    get_str(vpiFullName, object)
}

/// Returns the source `vpiFile` of an object.
pub fn obj_file(object: VpiHandle) -> String {
    get_str(vpiFile, object)
}

/// Returns the `vpiLineNo` of an object.
pub fn obj_line(object: VpiHandle) -> PLI_INT32 {
    get(vpiLineNo, object)
}

// ── Value ─────────────────────────────────────────────────────────────────────

/// Owned, safe representation of a VPI value read.
///
/// Replaces raw union access to [`VpiValue`]: `core` and `sim` must use
/// [`read_value`], never the raw `VpiValue`/`VpiValueData` types.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueData {
    /// `vpiSuppressVal` / no value / unknown format.
    None,
    /// `vpiScalarVal` — one of the `vpi0`..`vpiDontCare` constants.
    Scalar(i32),
    /// `vpiIntVal` — signed integer.
    Int(i64),
    /// `vpiUIntVal` — unsigned integer (UHDM extension).
    UInt(u64),
    /// `vpiRealVal` — double-precision real.
    Real(f64),
    /// `vpiStringVal` — string value.
    Str(String),
    /// `vpiBinStrVal` — "0/1/x/z" digits.
    Bin(String),
    /// `vpiOctStrVal` — "0-7/x/z" digits.
    Oct(String),
    /// `vpiDecStrVal` — decimal digits.
    Dec(String),
    /// `vpiHexStrVal` — "0-f/x/z" digits.
    Hex(String),
    /// `vpiVectorVal` — 4-state vector as (aval, bval) word pairs, LSB first.
    Vector(Vec<(u32, u32)>),
}

/// Read an object's current value.
///
/// All data is copied into owned Rust storage; the returned [`ValueData`]
/// shares nothing with UHDM.  Callers need no `unsafe`.  Uses `vpi_get_value`
/// internally.
pub fn read_value(expr: VpiHandle) -> ValueData {
    // SAFETY: `vpi_get_value` fills the stack `VpiValue` and selects `format`
    // to tell us which union member is active.  String members (`str_`) point
    // at NUL-terminated UHDM-owned storage and are copied out before this
    // function returns; the vector member points at an array of `VpiVecval`
    // words (4-state encoding: aval/bval pair `ab` = 00→0, 10→1, 11→X, 01→Z),
    // of which at most two 32-bit words are read because v1 constant values
    // are at most 64 bits wide.
    unsafe {
        let mut v = VpiValue {
            format: 0,
            value: VpiValueData { integer: 0 },
        };
        vpi_get_value(expr, &mut v);
        let copy_str = |p: *mut PLI_BYTE8| -> String {
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        };
        match v.format {
            vpiBinStrVal => ValueData::Bin(copy_str(v.value.str_)),
            vpiOctStrVal => ValueData::Oct(copy_str(v.value.str_)),
            vpiDecStrVal => ValueData::Dec(copy_str(v.value.str_)),
            vpiHexStrVal => ValueData::Hex(copy_str(v.value.str_)),
            vpiScalarVal => ValueData::Scalar(v.value.scalar),
            vpiIntVal => ValueData::Int(v.value.integer),
            vpiUIntVal => ValueData::UInt(v.value.uint),
            vpiRealVal => ValueData::Real(v.value.real),
            vpiStringVal => ValueData::Str(copy_str(v.value.str_)),
            vpiVectorVal => {
                let ptr = v.value.vector;
                if ptr.is_null() {
                    ValueData::None
                } else {
                    let size = vpi_get(vpiSize, expr);
                    let words = if size <= 0 {
                        1
                    } else {
                        (size as usize).div_ceil(32).clamp(1, 2)
                    };
                    let mut vec = Vec::with_capacity(words);
                    for i in 0..words {
                        let w = ptr.add(i).read();
                        vec.push((w.aval, w.bval));
                    }
                    ValueData::Vector(vec)
                }
            }
            _ => ValueData::None,
        }
    }
}

/// Reads the current value of a VPI object into `value_p`.
///
/// Low-level accessor kept public for compatibility; `core`/`sim` must use
/// [`read_value`] instead of touching the raw union.
pub fn get_value(expr: VpiHandle, value_p: &mut VpiValue) {
    unsafe {
        vpi_get_value(expr, value_p as *mut _);
    }
}

/// Writes a value to a VPI object. Returns a scheduled-event handle or null.
pub fn put_value(
    object: VpiHandle,
    value_p: &mut VpiValue,
    time_p: Option<&mut VpiTime>,
    flags: PLI_INT32,
) -> Option<OwnedHandle> {
    let tp = time_p.map_or(std::ptr::null_mut(), |t| t as *mut _);
    OwnedHandle::new(unsafe { vpi_put_value(object, value_p as *mut _, tp, flags) })
}

// ── Error checking ────────────────────────────────────────────────────────────

/// Returns the last VPI error information, or `None` if no error is pending.
pub fn chk_error() -> Option<VpiErrorInfo> {
    let mut info = std::mem::MaybeUninit::<VpiErrorInfo>::uninit();
    let has_error = unsafe { vpi_chk_error(info.as_mut_ptr()) };
    if has_error != 0 {
        Some(unsafe { info.assume_init() })
    } else {
        None
    }
}

// ── I/O ───────────────────────────────────────────────────────────────────────

/// Flushes all VPI output channels.
pub fn flush() {
    unsafe {
        vpi_flush();
    }
}

/// Opens a multi-channel descriptor file. Returns the MCD value, or 0 on failure.
pub fn mcd_open(file_name: &str) -> PLI_UINT32 {
    match CString::new(file_name) {
        Ok(s) => unsafe { vpi_mcd_open(s.as_ptr() as *mut _) },
        Err(_) => 0,
    }
}

/// Closes a multi-channel descriptor.
pub fn mcd_close(mcd: PLI_UINT32) -> PLI_UINT32 {
    unsafe { vpi_mcd_close(mcd) }
}
