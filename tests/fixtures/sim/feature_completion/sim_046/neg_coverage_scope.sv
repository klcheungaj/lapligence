// A generate scope is no module instance: the hierarchy argument of
// $coverage_get names an instance, $root or a definition string
// (IEEE 1800-2009 40.3.2.1).
module tb;
  logic clk = 0;
  for (genvar i = 0; i < 1; i++) begin : g
    ap: assert property (@(posedge clk) 1'b1);
  end
  initial begin
    $display("%0d", $coverage_get(`SV_COV_ASSERTION, `SV_COV_HIER, g[0]));
  end
endmodule
