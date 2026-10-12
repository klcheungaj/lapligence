// SV 1800-2009 14.16: a compound assignment is not a synchronous drive, also
// through a virtual interface.
interface bus_if (input bit clk);
  logic [7:0] b;
  clocking sb @(posedge clk);
    output b;
  endclocking
endinterface
module tb;
  bit clk;
  bus_if bi (clk);
  virtual bus_if v;
  initial begin
    v = bi;
    v.sb.b += 8'h1;
    $finish;
  end
endmodule
