// SIM-019 negative: reductions apply to integral values only (SV 7.12.3).
module tb;
  real r[$];
  real t;
  initial begin
    r.push_back(1.5);
    t = r.sum() with (item * 2.0);
  end
endmodule
