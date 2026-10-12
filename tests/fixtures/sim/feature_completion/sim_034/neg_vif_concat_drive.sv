// SV 1800-2009 14.16: "concatenation is not allowed" in a clockvar_expression,
// also through a virtual interface.
interface bus_if (input bit clk);
  logic [7:0] b, c;
  clocking sb @(posedge clk);
    output b, c;
  endclocking
endinterface
module tb;
  bit clk;
  bus_if bi (clk);
  virtual bus_if v;
  initial begin
    v = bi;
    {v.sb.b, v.sb.c} <= 16'h1;
    $finish;
  end
endmodule
