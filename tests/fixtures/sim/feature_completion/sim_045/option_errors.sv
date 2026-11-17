// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/option_errors.sv
// SIM-045: a negative option.weight or type_option.weight (Table 19-1,
// Table 19-3) is a run-time error located at the option, in the declaration
// and in a procedural write; the option keeps its value and the run goes on.
// Expected values are derived in readme.md.
module tb;
  bit [1:0] x;
  int bad = -1;

  covergroup g(int w);
    option.weight = w;
    cp: coverpoint x;
  endgroup

  g a;

  initial begin
    x = 2;
    a = new(-1);
    #1;
    a.cp.option.weight = bad;
    g::type_option.weight = bad;
    $display("weight %0d cp %0d type %0d", a.option.weight, a.cp.option.weight,
             g::type_option.weight);
    $finish;
  end
endmodule
