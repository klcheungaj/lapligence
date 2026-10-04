// Resource limit: a with key reads each 2,097,184-bit row as one item value,
// above the 1,048,575-bit packed capacity. Reversing the rows stays legal.
module tb;
  localparam int N = 65537;
  bit [31:0] rows [0:1][N];
  initial begin
    rows.reverse();
    rows.sort() with (item[0]);
    $finish;
  end
endmodule
