// Decision A17-D9: the order of the attempt callbacks of one clock tick.
// The events are grouped by instance: the instances below a top-level
// module in hierarchy order (each with its own assertions in declaration
// order), then the top-level module's own assertions. Within an instance
// the attempts that started later come first: the start of every new
// attempt, each followed directly by its conclusion when it concludes in
// its own tick, then the conclusions of older attempts, the most recent
// attempt first and, for one start time, in declaration order. Clock ticks:
// 5, 15, 25, 35; a is 1 throughout, b becomes 1 at 22.
//
// IEEE 1800-2009 39.4.2 (SystemVerilog-1800-2009.txt L65666): "cbAssertionStart.
// An assertion attempt has started. For most assertions, one attempt starts
// each and every clock tick." The clause does not order the callbacks of
// one tick; 4.7 (L3465-L3466): "active events can be taken off the Active
// or Reactive event region and processed in any order." The order is
// industry-standard behaviour.
//
// Build A17-D9_tick_order.c as a VPI application (llg: static archive given
// to --dpi-lib; `task $d9()` declared as a system task).
`timescale 1ns/1ns
module leaf(input logic clk, input logic a, input logic b);
    l1: assert property (@(posedge clk) a |=> b);
endmodule

module tb;
    logic clk = 0, a = 1, b = 0;
    always #5 clk = ~clk;
    t1: assert property (@(posedge clk) a |=> b);
    t2: cover property (@(posedge clk) a ##[1:3] b);
    t3: assert property (@(posedge clk) b);
    leaf u1(clk, a, b);
    leaf u2(clk, a, b);
    initial begin
        $d9;
        #22 b = 1'b1;
        #20 $finish;
    end
endmodule
