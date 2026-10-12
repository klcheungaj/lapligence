// SV 1800-2009 14.16: only clocking outputs and inouts are driven; driving an
// input clockvar through a virtual interface is an error.
interface bus_if (input bit clk);
  logic [7:0] a;
  clocking sb @(posedge clk);
    input a;
  endclocking
endinterface
module tb;
  bit clk;
  bus_if b (clk);
  virtual bus_if v;
  initial begin
    v = b;
    v.sb.a <= 8'h1;
    $finish;
  end
endmodule
