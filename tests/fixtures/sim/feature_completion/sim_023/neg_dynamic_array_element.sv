// SV 6.21: elements of dynamically sized variables shall not be written with
// procedural continuous assignments; industry-standard behaviour rejects a
// dynamic array element as a force target.
module tb;
  int dq[];
  initial begin
    dq = new[2];
    force dq[1] = 88;
  end
endmodule
