// Illegal weight type: the weights are integral values.
module tb;
  real weight = 1.5;
  int x;
  initial begin
    randcase
      weight : x = 1;
      1 : x = 2;
    endcase
    $finish;
  end
endmodule
