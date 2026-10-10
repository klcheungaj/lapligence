// SV 6.21: elements of dynamically sized variables shall not be written with
// procedural continuous assignments.
module tb;
  logic [3:0] q[$];
  initial begin
    q.push_back(4'h0);
    force q[0] = 4'h1;
  end
endmodule
