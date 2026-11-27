// Decision CS-D4: a release through vpi_put_value (vpiReleaseFlag) of a
// variable that only the action blocks of an assertion, a declaration
// initializer or procedures write keeps the forced value until its next
// procedural assignment; the release runs no action block. A variable a
// continuous assignment drives takes the driven value again.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64189-64191) makes a
// vpiReleaseFlag put the release of 10.6.2. 10.6.2 L13390-13393: "When
// released, then if the variable is not driven by a continuous assignment
// and does not currently have an active assign procedural continuous
// assignment, the variable shall not immediately change value and shall
// maintain its current value until the next procedural assignment to the
// variable is executed." 16.15.1 L26430-26432: "When the property for the
// assert statement is evaluated to be true, the pass statements of the
// action block are executed."
//
// Build CS-D4_vpi_release_keeps_action_block_writes.c as a VPI application
// (llg: static archive given to --dpi-lib; `task $d4a(int enable)` declared
// as a system task). $d4a(1) forces tb.passes, tb.fails, tb.init_v and tb.y
// to 100, 90, 80 and 70; $d4a(0) releases them.
`timescale 1ns / 1ns
module tb;
  logic clk = 0;
  always #5 clk = ~clk;
  int passes = 0;
  int fails = 0;
  int init_v = 5;
  logic a = 1;
  int y;
  assign y = 3;
  assert property (@(posedge clk) a) passes++; else fails++;
  initial begin
    #12 $display("%0t passes=%0d fails=%0d init_v=%0d y=%0d", $time, passes, fails, init_v, y);
    $d4a(1);
    #1 $d4a(0);
    #0 $display("%0t released passes=%0d fails=%0d init_v=%0d", $time, passes, fails, init_v);
    #10 $display("%0t passes=%0d fails=%0d init_v=%0d y=%0d", $time, passes, fails, init_v, y);
    $finish(0);
  end
endmodule
