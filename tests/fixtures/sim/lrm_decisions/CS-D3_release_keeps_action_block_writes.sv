// Decision CS-D3: a variable that only the action blocks of an assertion, a
// declaration initializer or procedures write keeps the forced value when it
// is released, until its next procedural assignment; the release runs no
// action block. A variable a continuous assignment drives takes the driven
// value again.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13390-13393): "When
// released, then if the variable is not driven by a continuous assignment
// and does not currently have an active assign procedural continuous
// assignment, the variable shall not immediately change value and shall
// maintain its current value until the next procedural assignment to the
// variable is executed." L13393-13395: "Releasing a variable that is driven
// by a continuous assignment [...] shall reestablish that assignment".
// 16.15.1 L26430-26432: "When the property for the assert statement is
// evaluated to be true, the pass statements of the action block are
// executed. When the property for the assert statement is evaluated to be
// false, the fail statements of the action_block are executed."
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
    force passes = 100;
    force fails = 50;
    force init_v = 60;
    force y = 80;
    #1 release passes;
    release fails;
    release init_v;
    release y;
    $display("%0t released passes=%0d fails=%0d init_v=%0d y=%0d", $time, passes, fails, init_v,
             y);
    #10 $display("%0t passes=%0d fails=%0d init_v=%0d y=%0d", $time, passes, fails, init_v, y);
    $finish(0);
  end
endmodule
