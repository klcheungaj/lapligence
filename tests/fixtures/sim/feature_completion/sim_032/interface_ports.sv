// SIM-032: program ports use the module port mechanism, including interface
// and modport ports (IEEE 1800-2009 24.3, 24.3.1).
interface bus_if(input logic clk);
  logic [7:0] data;
  logic valid;
  modport tb_mp(input clk, output data, output valid);
endinterface

program drv(bus_if.tb_mp b);
  initial begin
    b.valid = 1'b0;
    @(posedge b.clk) b.data = 8'h3c;
    b.valid = 1'b1;
    @(posedge b.clk) b.data = 8'h4d;
    @(posedge b.clk) $display("drv done t=%0d", $time);
  end
endprogram

module tb;
  logic clk = 1'b0;
  always #5 clk = ~clk;
  bus_if bi(clk);
  drv d0(bi.tb_mp);
  always @(posedge clk) $display("dut valid=%b data=%h t=%0d", bi.valid, bi.data, $time);
endmodule
