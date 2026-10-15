// SIM-046: coverage queries and a save in a final procedure read live
// coverage records (IEEE 1800-2009 9.2.3: final procedures run before the
// simulation ends). Regression: the records were released before the final
// procedures ran. Derivations: readme.md.
module tb;
  bit [3:0] v;
  logic clk = 0;
  covergroup cg;
    cp: coverpoint v { bins lo = {[0:7]}; bins hi = {[8:15]}; }
  endgroup
  cg c = new;
  always #5 clk = ~clk;
  ap: assert property (@(posedge clk) 1'b1);

  initial begin
    v = 1;
    c.sample();
    #10 $finish;
  end

  final begin
    $display("final inst %0.2f type %0.2f total %0.2f", c.get_inst_coverage(),
             cg::get_coverage(), $get_coverage());
    $display("final assertions %0d/%0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, $root),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, $root));
    $display("final save %0d", $coverage_save(`SV_COV_ASSERTION, "final.db"));
  end
endmodule
