// ADV-026 decision S26-D17 in Verilog-2001: a reg vector holding a string is a
// "string argument" (IEEE 1364-2001 C.13), as are a parameter and a
// concatenation of literals.
module tb;
  parameter IMAGE = "11 22";
  reg [7:0] mem [0:3];
  reg [47:0] image;
  initial begin
    $sreadmemh(mem, 0, 1, IMAGE);
    image = "33 44";
    $sreadmemh(mem, 2, 3, image);
    $display("a %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    image = "@3 55";
    $sreadmemh(mem, 0, 3, image, {"@0 6", "6"});
    $display("b %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
  end
endmodule
