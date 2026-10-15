// Decision S46-D9: the assertion coverage items are the assertion statement
// instances: concurrent assert, assume and cover property, cover sequence,
// and immediate and deferred immediate assertions. A procedural expect and
// a sequence used only through .triggered (an end point) are not items.
//
// IEEE 1800-2009 40.2.2 (SystemVerilog-1800-2009.txt L66035-66036):
// "assertion coverage: For each assertion, whether it has had at least one
// success." 39.3.2 (assertion API) lists the assertion types; expect is a
// procedural statement (16.17) and a sequence end point is no assertion.
module tb;
  logic clk = 0, a = 1;
  always #5 clk = ~clk;
  sequence s;
    @(posedge clk) a;
  endsequence
  pa: assert property (@(posedge clk) a);
  qa: assume property (@(posedge clk) a);
  ca: cover property (@(posedge clk) a);
  cs: cover sequence (@(posedge clk) a ##1 a);
  initial begin
    im: assert (a);
    df: assert #0 (a);
    expect (@(posedge clk) a);
    wait (s.triggered);
    $display("items %0d", $coverage_get_max(`SV_COV_ASSERTION, `SV_COV_HIER, $root));
    $finish;
  end
endmodule
