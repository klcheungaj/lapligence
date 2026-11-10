`timescale 1ns/1ns
// SV 1800-2009 14.16, 25.9: selects and whole drives of clockvars bound to a
// part-select, through a virtual interface, on a wire with another driver and
// on a variable (FIN-001 K2).
interface net_if (input bit clk);
  wire [7:0] w;
  logic [7:0] drv = 8'hzz;
  assign w = drv;
  clocking cb @(posedge clk);
    output x = w[5:2];
  endclocking
endinterface
interface var_if (input bit clk);
  logic [7:0] s;
  clocking cb @(posedge clk);
    output x = s[5:2];
  endclocking
endinterface
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  net_if ni (clk);
  var_if vi (clk);
  virtual net_if n = ni;
  virtual var_if v = vi;
  initial begin
    vi.s = 8'h00;
    #6 n.cb.x[3:2] <= 2'b10;
    v.cb.x[4] <= 1'b1;
    #10 $display("%0t w=%b s=%b", $time, ni.w, vi.s);
    n.cb.x <= 4'b0101;
    v.cb.x[2] <= 1'b1;
    #10 $display("%0t w=%b s=%b", $time, ni.w, vi.s);
    ni.drv = 8'b1111_0000;
    #1 $display("%0t w=%b", $time, ni.w);
    n.cb.x[5] <= 1'bz;
    v.cb.x[5:4] <= 2'b10;
    #10 $display("%0t w=%b s=%b", $time, ni.w, vi.s);
    $finish;
  end
endmodule
