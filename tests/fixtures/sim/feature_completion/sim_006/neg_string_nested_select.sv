module tb;
  int sq[string][$];
  initial begin
    sq["a"] = '{1, 2};
    sq["a"][1] = 3;
  end
endmodule
