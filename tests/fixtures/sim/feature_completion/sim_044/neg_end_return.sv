// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_end_return.sv
// SIM-044 negative: an end block event on a function that may return before its last statement is rejected at the covergroup (IEEE 1800-2009 19.3).
module tb;
  int n;
  function automatic int f(int k);
    if (k > 1) return 2;
    n = k;
    return 1;
  endfunction
  covergroup g @@(end f);
    coverpoint n;
  endgroup
  g c = new;
  initial begin
    n = f(0);
    $finish;
  end
endmodule
