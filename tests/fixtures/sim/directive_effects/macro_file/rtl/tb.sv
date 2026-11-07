// IEEE 1800-2009 22.13 (SystemVerilog-1800-2009.txt L39919-39921): "`__FILE__
// expands to the name of the current input file, in the form of a string
// literal. This is the path by which a tool opened the file, not the short name
// specified in `include or as a tool's input file name argument." The test runs
// this file as ../../src/rtl/tb.sv with -I ../../src/inc -I ../../src/deep/:
// each file reports the path it was given or found by (the include directory
// as given, joined with the `include name), a macro reports the file it is
// expanded in, and `line renames the file. Matches industry-standard behaviour.
`include "a.svh"
`define HERE_TB `__FILE__
module tb;
  initial begin
    $display("tb %s", `__FILE__);
    $display("macro from include used in tb: %s", `WHERE_A);
    $display("macro from b used in tb: %s", `WHERE_B);
    show_a();
    show_b();
    show_tb_macro();
`line 100 "renamed.sv" 0
    $display("line %s %0d", `__FILE__, `__LINE__);
    $finish;
  end
endmodule
