// ADV-026 decision S26-D17 in Verilog-2001: a reg vector holding a string is not
// a string literal (IEEE 1364-2001 C.13 says "string arguments"); rejected.
module tb;
  reg [7:0] mem [0:3];
  reg [47:0] image;
  initial begin
    image = "11 22";
    $sreadmemh(mem, 0, 1, image);
  end
endmodule
