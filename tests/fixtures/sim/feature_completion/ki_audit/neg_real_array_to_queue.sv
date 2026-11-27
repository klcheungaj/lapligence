// Remaining items of unpacked array concatenations: a whole fixed array of
// reals assigned to a queue.
module tb;
  real r2[2] = '{1.0, 2.0};
  real rq[$];
  initial begin
    rq = r2;
    $display("%p", rq);
  end
endmodule
