/* ADV-019 A03 catalog walker (IEEE 1800-2009 37.3, 38.6, 38.11, 38.18,
 * 38.23). From a NULL reference it walks every object reachable through
 * the probed relations and iterations, probing at most PER_TYPE objects of
 * each type. On each probed object it calls vpi_get and vpi_get_str with
 * every property probe, vpi_handle and vpi_iterate with every relation
 * probe (so a one-to-one relation is also tried as an iteration and the
 * reverse) and vpi_get_value with vpiObjTypeVal. It prints one line per
 * answer that raised no error:
 *   obj <type> <name>
 *   + <routine> <type> <probe> <result>
 * and one "!" line per answer that raised an error but still returned a
 * value, a handle or an iterator (38.6: vpiUndefined on error). The Rust
 * test compares the "+" lines with src/sim/rt/vpi_catalog.txt; the probe
 * tables must list every name the catalog and the runtime dispatch name,
 * which tests/runtime_vpi_catalog.rs checks. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

#define PER_TYPE 1000
#define QUEUE 16384
#define TYPES 1024

typedef struct {
    const char* name;
    PLI_INT32 value;
} probe_t;

static const probe_t property_probes[] = {
    {"vpiAccessType", vpiAccessType},
    {"vpiActive", vpiActive},
    {"vpiAllocScheme", vpiAllocScheme},
    {"vpiAlwaysType", vpiAlwaysType},
    {"vpiArray", vpiArray},
    {"vpiArrayMember", vpiArrayMember},
    {"vpiArrayType", vpiArrayType},
    {"vpiAssertAttemptCovered", vpiAssertAttemptCovered},
    {"vpiAssertCoverage", vpiAssertCoverage},
    {"vpiAssertDisableCovered", vpiAssertDisableCovered},
    {"vpiAssertFailureCovered", vpiAssertFailureCovered},
    {"vpiAssertKillCovered", vpiAssertKillCovered},
    {"vpiAssertSuccessCovered", vpiAssertSuccessCovered},
    {"vpiAssertVacuousSuccessCovered", vpiAssertVacuousSuccessCovered},
    {"vpiAutomatic", vpiAutomatic},
    {"vpiBlocking", vpiBlocking},
    {"vpiCaseType", vpiCaseType},
    {"vpiCell", vpiCell},
    {"vpiCellInstance", vpiCellInstance},
    {"vpiChargeStrength", vpiChargeStrength},
    {"vpiClassType", vpiClassType},
    {"vpiColumn", vpiColumn},
    {"vpiConfig", vpiConfig},
    {"vpiConnByName", vpiConnByName},
    {"vpiConstType", vpiConstType},
    {"vpiConstantSelect", vpiConstantSelect},
    {"vpiConstantVariable", vpiConstantVariable},
    {"vpiCoverMax", vpiCoverMax},
    {"vpiCovered", vpiCovered},
    {"vpiCoveredCount", vpiCoveredCount},
    {"vpiDPICIdentifier", vpiDPICIdentifier},
    {"vpiDPICStr", vpiDPICStr},
    {"vpiDPIContext", vpiDPIContext},
    {"vpiDPIPure", vpiDPIPure},
    {"vpiDataPolarity", vpiDataPolarity},
    {"vpiDecompile", vpiDecompile},
    {"vpiDefAttribute", vpiDefAttribute},
    {"vpiDefDecayTime", vpiDefDecayTime},
    {"vpiDefDelayMode", vpiDefDelayMode},
    {"vpiDefFile", vpiDefFile},
    {"vpiDefLineNo", vpiDefLineNo},
    {"vpiDefName", vpiDefName},
    {"vpiDefNetType", vpiDefNetType},
    {"vpiDelayType", vpiDelayType},
    {"vpiDirection", vpiDirection},
    {"vpiDistType", vpiDistType},
    {"vpiEdge", vpiEdge},
    {"vpiEndColumn", vpiEndColumn},
    {"vpiEndLine", vpiEndLine},
    {"vpiExpanded", vpiExpanded},
    {"vpiExplicitName", vpiExplicitName},
    {"vpiExplicitScalared", vpiExplicitScalared},
    {"vpiExplicitVectored", vpiExplicitVectored},
    {"vpiFile", vpiFile},
    {"vpiFsmStateCoverage", vpiFsmStateCoverage},
    {"vpiFullName", vpiFullName},
    {"vpiFuncType", vpiFuncType},
    {"vpiGeneric", vpiGeneric},
    {"vpiImplicitDecl", vpiImplicitDecl},
    {"vpiIndexedPartSelectType", vpiIndexedPartSelectType},
    {"vpiInputEdge", vpiInputEdge},
    {"vpiIsClockInferred", vpiIsClockInferred},
    {"vpiIsConstraintEnabled", vpiIsConstraintEnabled},
    {"vpiIsCoverSequence", vpiIsCoverSequence},
    {"vpiIsDeferred", vpiIsDeferred},
    {"vpiIsMemory", vpiIsMemory},
    {"vpiIsRandomized", vpiIsRandomized},
    {"vpiIteratorType", vpiIteratorType},
    {"vpiLibrary", vpiLibrary},
    {"vpiLineNo", vpiLineNo},
    {"vpiLocalParam", vpiLocalParam},
    {"vpiLocalVarDecls", vpiLocalVarDecls},
    {"vpiMethod", vpiMethod},
    {"vpiModPathHasIfNone", vpiModPathHasIfNone},
    {"vpiName", vpiName},
    {"vpiNetDeclAssign", vpiNetDeclAssign},
    {"vpiNetType", vpiNetType},
    {"vpiObjId", vpiObjId},
    {"vpiOffset", vpiOffset},
    {"vpiOpStrong", vpiOpStrong},
    {"vpiOpType", vpiOpType},
    {"vpiOutputEdge", vpiOutputEdge},
    {"vpiPacked", vpiPacked},
    {"vpiPackedArrayMember", vpiPackedArrayMember},
    {"vpiPathType", vpiPathType},
    {"vpiPolarity", vpiPolarity},
    {"vpiPortIndex", vpiPortIndex},
    {"vpiPortType", vpiPortType},
    {"vpiPrimType", vpiPrimType},
    {"vpiProtected", vpiProtected},
    {"vpiQualifier", vpiQualifier},
    {"vpiRandType", vpiRandType},
    {"vpiResolvedNetType", vpiResolvedNetType},
    {"vpiScalar", vpiScalar},
    {"vpiScheduled", vpiScheduled},
    {"vpiSigned", vpiSigned},
    {"vpiSize", vpiSize},
    {"vpiStartLine", vpiStartLine},
    {"vpiStatementCoverage", vpiStatementCoverage},
    {"vpiStrength0", vpiStrength0},
    {"vpiStrength1", vpiStrength1},
    {"vpiStructUnionMember", vpiStructUnionMember},
    {"vpiTagged", vpiTagged},
    {"vpiTchkType", vpiTchkType},
    {"vpiTermIndex", vpiTermIndex},
    {"vpiTimePrecision", vpiTimePrecision},
    {"vpiTimeUnit", vpiTimeUnit},
    {"vpiToggleCoverage", vpiToggleCoverage},
    {"vpiTop", vpiTop},
    {"vpiTopModule", vpiTopModule},
    {"vpiType", vpiType},
    {"vpiUnconnDrive", vpiUnconnDrive},
    {"vpiUnit", vpiUnit},
    {"vpiUserDefn", vpiUserDefn},
    {"vpiVector", vpiVector},
    {"vpiVirtual", vpiVirtual},
    {"vpiVisibility", vpiVisibility},
};
static const probe_t relation_probes[] = {
    {"vpiActiveTimeFormat", vpiActiveTimeFormat},
    {"vpiActual", vpiActual},
    {"vpiAliasStmt", vpiAliasStmt},
    {"vpiArgument", vpiArgument},
    {"vpiArrayNet", vpiArrayNet},
    {"vpiAssertion", vpiAssertion},
    {"vpiAttribute", vpiAttribute},
    {"vpiAutomatics", vpiAutomatics},
    {"vpiBaseExpr", vpiBaseExpr},
    {"vpiBaseTypespec", vpiBaseTypespec},
    {"vpiBit", vpiBit},
    {"vpiBitVar", vpiBitVar},
    {"vpiByteVar", vpiByteVar},
    {"vpiCallback", vpiCallback},
    {"vpiCaseItem", vpiCaseItem},
    {"vpiCasePropertyItem", vpiCasePropertyItem},
    {"vpiClassDefn", vpiClassDefn},
    {"vpiClassObj", vpiClassObj},
    {"vpiClassTypespec", vpiClassTypespec},
    {"vpiClockedSeq", vpiClockedSeq},
    {"vpiClockingBlock", vpiClockingBlock},
    {"vpiClockingEvent", vpiClockingEvent},
    {"vpiClockingIODecl", vpiClockingIODecl},
    {"vpiConcurrentAssertions", vpiConcurrentAssertions},
    {"vpiCondition", vpiCondition},
    {"vpiConstraint", vpiConstraint},
    {"vpiConstraintExpr", vpiConstraintExpr},
    {"vpiConstraintItem", vpiConstraintItem},
    {"vpiContAssign", vpiContAssign},
    {"vpiDefParam", vpiDefParam},
    {"vpiDefaultClocking", vpiDefaultClocking},
    {"vpiDefaultDisableIff", vpiDefaultDisableIff},
    {"vpiDelay", vpiDelay},
    {"vpiDelayControl", vpiDelayControl},
    {"vpiDelayDevice", vpiDelayDevice},
    {"vpiDerivedClasses", vpiDerivedClasses},
    {"vpiDisableCondition", vpiDisableCondition},
    {"vpiDistItem", vpiDistItem},
    {"vpiDriver", vpiDriver},
    {"vpiElemTypespec", vpiElemTypespec},
    {"vpiElement", vpiElement},
    {"vpiElseConst", vpiElseConst},
    {"vpiElseStmt", vpiElseStmt},
    {"vpiEnumConst", vpiEnumConst},
    {"vpiEnumTypespec", vpiEnumTypespec},
    {"vpiEnumVar", vpiEnumVar},
    {"vpiEventControl", vpiEventControl},
    {"vpiExpr", vpiExpr},
    {"vpiExtends", vpiExtends},
    {"vpiForIncStmt", vpiForIncStmt},
    {"vpiForInitStmt", vpiForInitStmt},
    {"vpiFrame", vpiFrame},
    {"vpiFsm", vpiFsm},
    {"vpiFsmStateExpression", vpiFsmStateExpression},
    {"vpiFsmStates", vpiFsmStates},
    {"vpiFunction", vpiFunction},
    {"vpiGate", vpiGate},
    {"vpiGateArray", vpiGateArray},
    {"vpiGenScope", vpiGenScope},
    {"vpiGenScopeArray", vpiGenScopeArray},
    {"vpiGenVar", vpiGenVar},
    {"vpiGlobalClocking", vpiGlobalClocking},
    {"vpiHighConn", vpiHighConn},
    {"vpiIODecl", vpiIODecl},
    {"vpiImport", vpiImport},
    {"vpiInTerm", vpiInTerm},
    {"vpiIndex", vpiIndex},
    {"vpiIndexTypespec", vpiIndexTypespec},
    {"vpiInitial", vpiInitial},
    {"vpiInputSkew", vpiInputSkew},
    {"vpiInstance", vpiInstance},
    {"vpiInstanceArray", vpiInstanceArray},
    {"vpiIntVar", vpiIntVar},
    {"vpiIntegerVar", vpiIntegerVar},
    {"vpiInterface", vpiInterface},
    {"vpiInterfaceArray", vpiInterfaceArray},
    {"vpiInterfaceDecl", vpiInterfaceDecl},
    {"vpiInterfaceTfDecl", vpiInterfaceTfDecl},
    {"vpiInternalScope", vpiInternalScope},
    {"vpiLeftRange", vpiLeftRange},
    {"vpiLetDecl", vpiLetDecl},
    {"vpiLhs", vpiLhs},
    {"vpiLoad", vpiLoad},
    {"vpiLocalDriver", vpiLocalDriver},
    {"vpiLocalLoad", vpiLocalLoad},
    {"vpiLongIntVar", vpiLongIntVar},
    {"vpiLoopVars", vpiLoopVars},
    {"vpiLowConn", vpiLowConn},
    {"vpiMatchItem", vpiMatchItem},
    {"vpiMember", vpiMember},
    {"vpiMemory", vpiMemory},
    {"vpiMemoryWord", vpiMemoryWord},
    {"vpiMessages", vpiMessages},
    {"vpiMethods", vpiMethods},
    {"vpiModDataPathIn", vpiModDataPathIn},
    {"vpiModPath", vpiModPath},
    {"vpiModPathIn", vpiModPathIn},
    {"vpiModPathOut", vpiModPathOut},
    {"vpiModport", vpiModport},
    {"vpiModule", vpiModule},
    {"vpiModuleArray", vpiModuleArray},
    {"vpiNamedBegin", vpiNamedBegin},
    {"vpiNamedEvent", vpiNamedEvent},
    {"vpiNamedEventArray", vpiNamedEventArray},
    {"vpiNamedFork", vpiNamedFork},
    {"vpiNet", vpiNet},
    {"vpiNetArray", vpiNetArray},
    {"vpiOperand", vpiOperand},
    {"vpiOrigin", vpiOrigin},
    {"vpiOutTerm", vpiOutTerm},
    {"vpiOutputSkew", vpiOutputSkew},
    {"vpiPackage", vpiPackage},
    {"vpiPackedArrayVar", vpiPackedArrayVar},
    {"vpiParamAssign", vpiParamAssign},
    {"vpiParameter", vpiParameter},
    {"vpiParent", vpiParent},
    {"vpiPathTerm", vpiPathTerm},
    {"vpiPattern", vpiPattern},
    {"vpiPort", vpiPort},
    {"vpiPortInst", vpiPortInst},
    {"vpiPrefix", vpiPrefix},
    {"vpiPrimTerm", vpiPrimTerm},
    {"vpiPrimitive", vpiPrimitive},
    {"vpiPrimitiveArray", vpiPrimitiveArray},
    {"vpiProcess", vpiProcess},
    {"vpiProgram", vpiProgram},
    {"vpiProgramArray", vpiProgramArray},
    {"vpiPropFormalDecl", vpiPropFormalDecl},
    {"vpiProperty", vpiProperty},
    {"vpiPropertyDecl", vpiPropertyDecl},
    {"vpiPropertyExpr", vpiPropertyExpr},
    {"vpiPropertySpec", vpiPropertySpec},
    {"vpiRange", vpiRange},
    {"vpiRealVar", vpiRealVar},
    {"vpiReg", vpiReg},
    {"vpiRegArray", vpiRegArray},
    {"vpiRepeatControl", vpiRepeatControl},
    {"vpiReturn", vpiReturn},
    {"vpiRhs", vpiRhs},
    {"vpiRightRange", vpiRightRange},
    {"vpiScope", vpiScope},
    {"vpiSeqFormalDecl", vpiSeqFormalDecl},
    {"vpiSequenceDecl", vpiSequenceDecl},
    {"vpiShortIntVar", vpiShortIntVar},
    {"vpiShortRealVar", vpiShortRealVar},
    {"vpiSimNet", vpiSimNet},
    {"vpiSolveAfter", vpiSolveAfter},
    {"vpiSolveBefore", vpiSolveBefore},
    {"vpiSpecParam", vpiSpecParam},
    {"vpiStmt", vpiStmt},
    {"vpiStructVar", vpiStructVar},
    {"vpiSwitch", vpiSwitch},
    {"vpiSwitchArray", vpiSwitchArray},
    {"vpiSysFuncCall", vpiSysFuncCall},
    {"vpiSysTaskCall", vpiSysTaskCall},
    {"vpiSysTfCall", vpiSysTfCall},
    {"vpiTableEntry", vpiTableEntry},
    {"vpiTask", vpiTask},
    {"vpiTaskFunc", vpiTaskFunc},
    {"vpiTchk", vpiTchk},
    {"vpiTchkDataTerm", vpiTchkDataTerm},
    {"vpiTchkNotifier", vpiTchkNotifier},
    {"vpiTchkRefTerm", vpiTchkRefTerm},
    {"vpiTchkTerm", vpiTchkTerm},
    {"vpiThread", vpiThread},
    {"vpiTimeQueue", vpiTimeQueue},
    {"vpiTimeVar", vpiTimeVar},
    {"vpiTypedef", vpiTypedef},
    {"vpiTypedefAlias", vpiTypedefAlias},
    {"vpiTypespec", vpiTypespec},
    {"vpiTypespecMember", vpiTypespecMember},
    {"vpiUdp", vpiUdp},
    {"vpiUdpArray", vpiUdpArray},
    {"vpiUdpDefn", vpiUdpDefn},
    {"vpiUnionVar", vpiUnionVar},
    {"vpiUse", vpiUse},
    {"vpiUserSystf", vpiUserSystf},
    {"vpiValueRange", vpiValueRange},
    {"vpiVarSelect", vpiVarSelect},
    {"vpiVariables", vpiVariables},
    {"vpiWaitingProcesses", vpiWaitingProcesses},
    {"vpiWeight", vpiWeight},
    {"vpiWidthExpr", vpiWidthExpr},
    {"vpiWith", vpiWith},
};

#define COUNT(a) ((int)(sizeof(a) / sizeof((a)[0])))

static vpiHandle queue[QUEUE];
static int queue_type[QUEUE];
static int head, tail;
static int admitted[TYPES];

static int type_of(vpiHandle h) {
    PLI_INT32 t = vpi_get(vpiType, h);
    return vpi_chk_error(NULL) || t < 0 || t >= TYPES ? -1 : (int)t;
}

static const char* label(vpiHandle h) {
    const char* s = vpi_get_str(vpiFullName, h);
    if (vpi_chk_error(NULL) || !s) s = vpi_get_str(vpiName, h);
    if (vpi_chk_error(NULL) || !s) s = "-";
    return s;
}

/* Keep h for probing unless PER_TYPE objects of its type are already
 * kept or it compares equal to one of them; otherwise release it. */
static void admit(vpiHandle h) {
    int t = type_of(h);
    if (t >= 0 && admitted[t] < PER_TYPE && tail < QUEUE) {
        for (int i = 0; i < tail; ++i)
            if (queue_type[i] == t && vpi_compare_objects(queue[i], h)) {
                vpi_release_handle(h);
                return;
            }
        queue[tail] = h;
        queue_type[tail++] = t;
        ++admitted[t];
        return;
    }
    vpi_release_handle(h);
}

static void probe(vpiHandle h, int t);

static void probe_iterate(vpiHandle h, int t, const probe_t* p) {
    vpiHandle it = vpi_iterate(p->value, h), item;
    if (vpi_chk_error(NULL)) {
        if (it) {
            vpi_printf("! iterate %d %s iterator\n", t, p->name);
            vpi_release_handle(it);
        }
        return;
    }
    int count = 0;
    if (it) {
        if (admitted[vpiIterator] == 0) {
            ++admitted[vpiIterator];
            probe(it, vpiIterator);
        }
        while ((item = vpi_scan(it))) {
            ++count;
            admit(item);
        }
    }
    vpi_printf("+ iterate %d %s %d\n", t, p->name, count);
}

static void probe(vpiHandle h, int t) {
    vpi_printf("obj %d %s\n", t, h ? label(h) : "NULL");
    for (int i = 0; h && i < COUNT(property_probes); ++i) {
        PLI_INT32 v = vpi_get(property_probes[i].value, h);
        if (!vpi_chk_error(NULL))
            vpi_printf("+ get %d %s %d\n", t, property_probes[i].name, (int)v);
        else if (v != vpiUndefined)
            vpi_printf("! get %d %s %d\n", t, property_probes[i].name, (int)v);
    }
    for (int i = 0; !h && i < COUNT(property_probes); ++i) {
        PLI_INT32 v = vpi_get(property_probes[i].value, NULL);
        if (!vpi_chk_error(NULL)) vpi_printf("+ get 0 %s %d\n", property_probes[i].name, (int)v);
    }
    for (int i = 0; h && i < COUNT(property_probes); ++i) {
        const char* s = vpi_get_str(property_probes[i].value, h);
        if (!vpi_chk_error(NULL))
            vpi_printf("+ str %d %s %s\n", t, property_probes[i].name, s ? s : "(null)");
        else if (s)
            vpi_printf("! str %d %s %s\n", t, property_probes[i].name, s);
    }
    for (int i = 0; i < COUNT(relation_probes); ++i) {
        vpiHandle r = vpi_handle(relation_probes[i].value, h);
        if (vpi_chk_error(NULL)) {
            if (r) {
                vpi_printf("! handle %d %s handle\n", t, relation_probes[i].name);
                vpi_release_handle(r);
            }
            continue;
        }
        vpi_printf("+ handle %d %s %d\n", t, relation_probes[i].name, r ? type_of(r) : 0);
        if (r) admit(r);
    }
    for (int i = 0; i < COUNT(relation_probes); ++i) probe_iterate(h, t, &relation_probes[i]);
    if (h) {
        s_vpi_value value;
        value.format = vpiObjTypeVal;
        vpi_get_value(h, &value);
        if (!vpi_chk_error(NULL)) vpi_printf("+ value %d vpi_get_value %d\n", t, (int)value.format);
    }
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    probe(NULL, 0);
    while (head < tail) {
        vpiHandle h = queue[head];
        int t = queue_type[head++];
        probe(h, t);
    }
    vpi_printf("done %d objects\n", tail);
    return 0;
}

/* $catalog_value returns 0; it gives the design a system function call. */
static PLI_INT32 zero(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 0;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &value, NULL, vpiNoDelay);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$catalog";
    task.calltf = walk;
    vpi_register_systf(&task);
    task.type = vpiSysFunc;
    task.sysfunctype = vpiIntFunc;
    task.tfname = (PLI_BYTE8*)"$catalog_value";
    task.calltf = zero;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
