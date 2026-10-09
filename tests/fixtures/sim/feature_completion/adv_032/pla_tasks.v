module tb;
  reg [1:3] mem [1:4];
  reg [1:3] in;
  reg [1:4] out;
  initial begin
    mem[1] = 3'b100;
    in = 3'b101;
    $async$and$array(mem, in, out);
    $async$nand$plane(mem, in, out);
    $async$or$array(mem, in, out);
    $async$nor$plane(mem, in, out);
    $sync$and$array(mem, in, out);
    $sync$nand$plane(mem, in, out);
    $sync$or$array(mem, in, out);
    $sync$nor$plane(mem, in, out);
    $display("%b", out);
    $finish;
  end
endmodule
