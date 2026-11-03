// FIN-001 G3 (F07-11): a queue of reals as an `inside` set item is legal
// (SV 11.4.13) but not supported: container set items must hold packed
// elements.
module tb;
  real rq[$] = {1.5, 2.5};
  real r = 2.5;
  initial begin
    $display("%0d", r inside {rq});
    $finish;
  end
endmodule
