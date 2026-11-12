// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_ref_actual.sv
// SIM-044 negative: the actual of a covergroup `ref` argument must be a whole
// static variable or net in llg; an unpacked array element is not (llg
// limitation).
module tb;
  bit [2:0] arr[4];
  covergroup g(ref bit [2:0] r);
    cr: coverpoint r;
  endgroup
  g c = new(arr[1]);
  initial $finish;
endmodule
