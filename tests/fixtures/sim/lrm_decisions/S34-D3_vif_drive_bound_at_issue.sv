// Decision S34-D3: a synchronous drive through a virtual interface drives the
// clockvar of the instance the handle names when the drive executes, with
// that instance's clocking event and skew. Assigning the handle another
// instance before the drive matures does not redirect it.
//
// IEEE 1800-2009 25.9 (SystemVerilog-1800-2009.txt L44891-44893):
//   "Virtual interface variables may be passed as arguments to tasks,
//   functions, or methods. A single virtual interface variable can thus
//   represent different interface instances at different times throughout the
//   simulation."
// IEEE 1800-2009 14.16 (L20144-20147):
//   "Such drive statements shall execute without blocking, but shall perform
//   their drive action as if they had executed at the time of the next
//   clocking event. The expression on the right-hand side of the drive
//   statement shall be evaluated immediately, but the processing of the drive
//   is delayed until the time of the next clocking event."
//
// The text does not say when the handle of a drive's target is resolved; llg
// resolves it when the drive executes, like an NBA target.
interface ifc (input bit clk);
  logic [7:0] x = 0;
  clocking cb @(posedge clk);
    output #1 x;
  endclocking
endinterface

module tb;
  bit c1 = 0, c2 = 0;
  ifc i1 (c1);
  ifc i2 (c2);
  virtual ifc v;
  always @(i1.x) $display("%0d i1.x=%0d", $time, i1.x);
  always @(i2.x) $display("%0d i2.x=%0d", $time, i2.x);
  initial begin
    v = i1;
    v.cb.x <= 8'd5;
    v = i2;
    v.cb.x <= ##1 8'd6;
    #2 c1 = 1;
    #3 c2 = 1;
    #2 $display("%0d v names i2: i1.x=%0d i2.x=%0d", $time, i1.x, i2.x);
    $finish;
  end
endmodule
