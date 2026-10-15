// Decision S46-D4: for $coverage_get, an assertion item is covered once it
// has had a nonvacuous success (a cover: a match), whatever failures it also
// had; vacuous successes do not cover it.
//
// IEEE 1800-2009 40.2.2 (SystemVerilog-1800-2009.txt L66035-66036):
// "assertion coverage: For each assertion, whether it has had at least one
// success." 40.3.2.3 (L66303-66305): "pos_num representing the current
// coverage level, i.e., the number of the coverable items that have been
// covered". Vacuity is defined in 16.13.6; whether a vacuous success counts
// is not stated, and vpiCovered (40.5.3, L66606-66607) additionally requires
// that the assertion never failed.
module tb;
  logic clk = 0, a = 0, b = 0;
  always #5 clk = ~clk;
  vac: assert property (@(posedge clk) a |-> b);
  mix: assert property (@(posedge clk) b) else $display("mix failed");
  hit: cover property (@(posedge clk) b);
  initial begin
    // Tick 1: a = 0, b = 0 (vac vacuous, mix fails); tick 2: b = 1.
    #10 b = 1'b1;
    #10 $display("covered %0d of %0d",
                 $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, tb),
                 $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, tb));
    $finish;
  end
endmodule
