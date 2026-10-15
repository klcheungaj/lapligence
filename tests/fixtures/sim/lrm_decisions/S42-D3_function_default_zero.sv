// Decision S42-D3: a system function whose calltf puts no value returns 0;
// a put to a system function call that is not active is ignored, without an
// error, and returns NULL.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64244-64247):
//   "Attempts to use vpi_put_value() with a handle to the system function
//   when the calltf routine is not active shall be ignored. Should the calltf
//   routine for a user-defined system function fail to put a value during its
//   execution, the default value of 0 will be applied."
// Whether an ignored put also sets the error status is not specified; llg
// sets none.
//
// Build S42-D3_function_default_zero.c as a VPI application (llg: static
// archive given to --dpi-lib; `function int $d3()` and `task $d3_late()`).
module tb;
    int r = 7;
    initial begin
        r = $d3();
        $display("r=%0d", r);
        $d3_late;
        $finish;
    end
endmodule
