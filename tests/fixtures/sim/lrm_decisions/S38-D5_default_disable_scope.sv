// Decision S38-D5: a `default disable iff` applies to the concurrent assert,
// assume and cover statements of its scope that have no disable iff of their
// own; an explicit `disable iff` overrides it, and a procedural expect does
// not inherit it.
//
// IEEE 1800-2009 16.16 (SystemVerilog-1800-2009.txt L27490-27491): "It
// provides a default disable condition to all concurrent assertions in the
// scope and subscopes of the default disable iff declaration." L27555-27556:
// "a) If an assertion has a disable iff clause, then the disable condition
// specified in this clause shall be used and any default disable iff
// declaration ignored for this assertion." The expect statement (16.18) is
// not a concurrent_assertion_statement (16.15, L26404); llg does not apply the
// default to it.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, s = 1'b0, t = 1'b0, kill = 1'b0;
  default disable iff (kill);
  a1: assert property (@(posedge clk) s |=> s_eventually [0:2] t)
    else $display("%0t a1 fail", $time);
  a2: assert property (@(posedge clk) disable iff (1'b0) s |=> s_eventually [0:2] t)
    else $display("%0t a2 fail", $time);
  initial begin
    #2;
    expect (@(posedge clk) s ##1 !t ##1 !t) $display("%0t e pass", $time);
    else $display("%0t e fail", $time);
  end
  initial begin
    s = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    s = 1'b0;
    repeat (4) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $finish;
  end
  initial begin
    #17 kill = 1'b1;
    #1 kill = 1'b0;
  end
endmodule
