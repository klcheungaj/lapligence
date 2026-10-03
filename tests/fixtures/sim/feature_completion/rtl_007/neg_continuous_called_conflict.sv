// SV2009 6.5: a variable with a continuous driver cannot also be written by a
// procedural assignment, including one inside a function that another
// continuous assignment calls.
module tb;
  logic [7:0] x, y;
  int cnt;
  function automatic logic [7:0] counted(input logic [7:0] v);
    cnt = cnt + 1;
    return v;
  endfunction
  assign cnt = 0;
  assign y = counted(x);
endmodule
