// SV 10.6.2: a force target "shall not be a bit-select or a part-select of a
// variable", also of an element of an unpacked array (decision S23-D7).
module tb;
  logic [7:0] mem[2];
  initial begin
    mem[1] = 8'h22;
    force mem[1][3:0] = 4'hf;
  end
endmodule
