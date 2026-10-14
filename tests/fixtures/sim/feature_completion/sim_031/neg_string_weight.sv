// Illegal weight type: a string is not an integral value.
module tb;
  string weight = "3";
  int x;
  initial begin
    randcase
      weight : x = 1;
      1 : x = 2;
    endcase
    $finish;
  end
endmodule
