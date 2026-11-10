// SV 1800-2009 14.16: a select of a clockvar bound to an expression, driven
// through a virtual interface.
interface ifc (input bit clk);
  logic [7:0] s;
  clocking cb @(posedge clk);
    output x = s[3:0];
  endclocking
endinterface
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  ifc i (clk);
  virtual ifc v = i;
  initial begin
    i.s = 8'h00;
    #6 v.cb.x[1] <= 1'b1;
    #10 $display("%0t s=%b", $time, i.s);
    #4 v.cb.x[3:2] <= 2'b11;
    #10 $display("%0t s=%b", $time, i.s);
    $finish;
  end
endmodule
