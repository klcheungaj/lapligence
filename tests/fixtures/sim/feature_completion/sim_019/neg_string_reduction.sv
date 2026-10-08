// SIM-019 negative: string elements have no reduction without an integral with expression.
module tb;
  string s[$];
  string t;
  initial begin
    s.push_back("a");
    t = s.sum();
  end
endmodule
