// SIM-046 $coverage_merge of foreign, corrupt and incomplete databases
// (IEEE 1800-2009 40.3.2.4): the call returns SV_COV_ERROR, reports a
// located run-time error and changes no counter. Derivations: readme.md.
module tb;
  parameter int N = 2;
  logic clk = 0;
  string name;
  always #5 clk = ~clk;
  for (genvar i = 0; i < N; i++) begin : g
    ap: assert property (@(posedge clk) 1'b1);
  end

  initial begin
    if ($value$plusargs("merge=%s", name))
      $display("merge %0d", $coverage_merge(`SV_COV_ASSERTION, name));
    if ($value$plusargs("toggle=%s", name))
      $display("toggle %0d", $coverage_merge(`SV_COV_TOGGLE, name));
    $display("merged %0d/%0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, $root),
             $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, $root));
    #10;
    if ($value$plusargs("save=%s", name))
      $display("save %0d", $coverage_save(`SV_COV_ASSERTION, name));
    $finish;
  end
endmodule
