// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/option_errors.sv
// SIM-045: a negative option.weight (Table 19-1) set by the declaration's
// option assignment when the instance is constructed is a run-time error
// located at the option, and it ends the simulation: nothing after the
// construction runs, the final procedure runs and the exit status is 1.
// Expected values are derived in readme.md.
module tb;
  bit [1:0] x;

  covergroup g(int w);
    option.weight = w;
    cp: coverpoint x;
  endgroup

  g a;

  initial begin
    x = 2;
    $display("before new");
    a = new(-1);
    $display("after new weight %0d", a.option.weight);
    $finish;
  end

  final $display("final");
endmodule
