#include "llg_rt.c"
#include "llg_vpi.c"
#include "probe.h"

static PLI_INT32 function_size(PLI_BYTE8* data) { (void)data; return 129; }
static PLI_INT32 function_call(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    s_vpi_value result = {.format = vpiIntVal};
    result.value.integer = 42;
    CHECK(vpi_put_value(call, &result, NULL, vpiNoDelay) == call);
    result.value.integer = 43;
    CHECK(vpi_put_value(call, &result, NULL, vpiNoDelay) == call);
    return 0;
}

int main(void) {
    for (unsigned cycle = 0; cycle < 10; ++cycle) {
        llg_rt_init();
        g.current_region = LLG_REGION_ACTIVE;
        sv4_t target = sv4_zero(129, 0);
        sv4_t wide = sv4_zero(65537, 0);
        llg_vpi_model_object_t objects[] = {
            {.type = vpiModule, .name = "top", .full_name = "top"},
            {.type = vpiReg, .name = "value", .full_name = "top.value", .width = 129,
             .packed = &target, .parent = &objects[0]},
            {.type = vpiReg, .name = "wide", .full_name = "top.wide", .width = 65537,
             .packed = &wide, .parent = &objects[0]},
        };
        CHECK(llg_vpi_model_init("top", objects, 3));
        vpiHandle handle = vpi_handle_by_name("top.value", NULL);
        vpiHandle wide_handle = vpi_handle_by_name("top.wide", NULL);
        CHECK(handle && wide_handle);
        s_vpi_vecval words[5] = {{0}};
        words[0].aval = 0x55;
        words[0].bval = 6;
        words[4].aval = 1;
        s_vpi_value input = {.format = vpiVectorVal};
        input.value.vector = words;
        for (unsigned i = 0; i < 1000; ++i) {
            CHECK(vpi_put_value(handle, &input, NULL, vpiNoDelay) == handle);
            s_vpi_value output = {.format = vpiVectorVal};
            vpi_get_value(handle, &output);
            CHECK(output.value.vector[0].aval == 0x55);
            CHECK(output.value.vector[0].bval == 6);
            CHECK(output.value.vector[4].aval == 1);
            CHECK(target.bits[0] == 0x51 && target.x[0] == 4 && target.z[0] == 2);
            CHECK(value_test_live() == 2);
        }
        wide.bits[1024] = 1;
        s_vpi_value text = {.format = vpiBinStrVal};
        vpi_get_value(wide_handle, &text);
        CHECK(strlen(text.value.str) == 65537 && text.value.str[0] == '1');
        CHECK(value_test_live() == 2);
        s_vpi_systf_data registration = {0};
        registration.type = vpiSysFunc;
        registration.sysfunctype = vpiSizedFunc;
        registration.tfname = "$owned_value";
        registration.calltf = function_call;
        registration.sizetf = function_size;
        CHECK(vpi_register_systf(&registration));
        CHECK(llg_vpi_compile_call_site(1, "$owned_value", NULL, 0, 1));
        for (unsigned i = 0; i < 1000; ++i) {
            sv4_t result = llg_vpi_call_function_site(1, "$owned_value", NULL, 0, 129, 0);
            CHECK(result.width == 129);
            expect_number(result, 43);
            CHECK(value_test_live() == 2);
        }
        CHECK(!llg_vpi_failed());
        llg_vpi_shutdown();
        llg_rt_cleanup();
        sv4_destroy(&target);
        sv4_destroy(&wide);
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    puts("VPI vector/text conversion and returned owners: OK");
    return 0;
}
