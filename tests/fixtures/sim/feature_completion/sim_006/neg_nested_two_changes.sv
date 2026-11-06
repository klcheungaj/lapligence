module tb;
  int sq[string][$];
  string a = "a", b = "b";
  initial begin
    sq["a"] = '{1, 2};
    sq[b].push_back(sq[a].pop_front());
  end
endmodule
