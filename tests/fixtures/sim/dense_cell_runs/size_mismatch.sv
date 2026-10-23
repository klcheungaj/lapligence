// Nearest illegal form: a whole fixed-array copy between different sizes.
module tb;
  int A [10:1];
  int C [24:1];
  initial begin
    A = C;
    $display("%0d", A[1]);
  end
endmodule
