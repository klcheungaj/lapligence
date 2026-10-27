// A19-D12: an expression object reads its value as the design evaluates it.
// Implicit conversions are not objects: an operand keeps the size it is
// written with while the operation has its context size. A select outside
// the value or with an unknown index reads x; a select of a packed
// dimension is a vpiBitSelect of the element's size; a real expression has
// size 64. A function call has no value (vpiError).
module tb;
  logic [7:0] a = 8'hf0, b = 8'h20;
  logic [3:0] n = 4'bx;
  logic [7:0] mem [0:1];
  logic [1:0][3:0] pk = 8'h5a;
  real r = 0.25;
  logic [15:0] wide;
  logic y_bit, y_x;
  logic [7:0] y_mem;
  logic [3:0] y_pk;
  real y_real;
  int y_call;
  function automatic int one();
    return 1;
  endfunction
  assign wide = a + b;
  assign y_bit = a[9];
  assign y_x = a[n];
  assign y_mem = mem[n];
  assign y_pk = pk[0];
  assign y_real = r + 1;
  assign y_call = one() + 1;
  initial begin
    mem[0] = 8'h11;
    mem[1] = 8'h22;
    #1 $d12;
    $finish;
  end
endmodule
