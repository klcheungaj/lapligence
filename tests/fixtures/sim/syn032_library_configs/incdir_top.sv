`timescale 1ns/1ns
module incdir_top;
  wire [7:0] rtl_value;
  wire [7:0] gate_value;
  rtl_cell rtl_instance(rtl_value);
  gate_cell gate_instance(gate_value);
  initial begin
    #1;
    $display("incdir=%0d,%0d", rtl_value, gate_value);
    $finish;
  end
endmodule
