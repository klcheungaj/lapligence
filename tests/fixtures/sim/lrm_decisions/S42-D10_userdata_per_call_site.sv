// Decision S42-D10: user data belongs to one system task call site (the
// call instance in the source); every call handle obtained for that site,
// in compiletf or any calltf, refers to the same object and sees the same
// data; another site of the same task has its own data, NULL until set.
//
// IEEE 1800-2009 38.33 (SystemVerilog-1800-2009.txt L64124-64126):
//   "This routine will associate the value of the input userdata with the
//   specified user-defined system task or system function call handle. ...
//   The routine will return a value of 1 on success or a 0 if it fails."
// 38.14 (L62510-62511): "If no user data had been previously associated with
//   the object or if the routine fails, the return value shall be NULL."
// That a call handle names the source instance, so it is the same object on
// every invocation, is an llg reading of "system task instance" (L62502).
//
// Build S42-D10_userdata_per_call_site.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d10()` declared as a system task).
module tb;
    initial begin
        repeat (3) $d10;
        $d10;
        $finish;
    end
endmodule
