module tb;
  int sq[string][$];
  initial begin
    sq["a"] = '{1, 2};
    sq["b"].push_back(sq["a"].pop_front());
  end
endmodule
