// Operand order of record elements of queues and dynamic arrays: a nested
// assignment to an unpacked value selected with a call, in an operand.
module tb;
  int ua[2][2];
  int ub[2] = '{1, 2};
  int k;
  function automatic int f();
    k++;
    return 0;
  endfunction
  initial begin
    bit x;
    x = ((ua[f()] = ub) == ub);
    $display("%0d %0d", x, ua[0][1]);
  end
endmodule
