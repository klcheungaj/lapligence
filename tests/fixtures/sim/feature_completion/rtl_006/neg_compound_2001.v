// IEEE 1364-2001 has no operator assignments; they arrive with SystemVerilog.
module tb;
  reg [7:0] x;
  initial begin
    x = 1;
    x += 1;
  end
endmodule
