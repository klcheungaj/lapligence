// IEEE 1364-2001 has no array manipulation methods.
module tb;
  reg [7:0] a [0:3];
  reg [7:0] s;
  initial begin
    s = a.sum();
    $finish;
  end
endmodule
