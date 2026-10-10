// Decision S36-D7: a deferred assertion used as a module item re-executes
// when its action's by-value arguments change.
//
// IEEE 1800-2009 16.4.3 (SystemVerilog-1800-2009.txt L21341-21342): "A
// deferred assertion statement may also appear outside procedural code, used
// as a module_common_item. In such cases, it is treated as if it were
// contained in an always_comb procedure."
// 9.2.2.2.1 (L11184-11186): "The implicit sensitivity list of an always_comb
// includes the expansions of the longest static prefix of each variable or
// select expression that is read within the block".
//
// The action's argument `b` is read within the equivalent always_comb, so a
// later change of `b` in the same time step re-executes the assertion; the
// re-execution is a flush point (16.4.2) and the report carries the settled
// value. (llg formerly re-executed such a member only when its condition's
// inputs changed and printed "a1 fail b=1".)
module tb;
  logic a = 1'b1;
  int b = 0;
  a1: assert #0 (a) else $display("a1 fail b=%0d", b);
  initial begin
    #1 a = 1'b0;
    b = 1;
    #0 b = 2;
    #1 $finish;
  end
endmodule
