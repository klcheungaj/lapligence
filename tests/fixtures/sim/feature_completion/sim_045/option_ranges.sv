// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/option_ranges.sv
// SIM-045: procedural writes of the bounded coverage options. Without a
// define every value is in range and the run goes on; each define writes one
// value out of range, a run-time error located at the write that ends the
// simulation: the statements after it do not run, the final procedure runs
// and the exit status is 1. Expected values are derived in readme.md.
module tb;
  bit [1:0] x, y;
  int lo = 0, hi = 100, big = 2147483647;
`ifdef WEIGHT
  int bad = -1;
`elsif GOAL_HIGH
  int bad = 101;
`else
  int bad = -1;
`endif

  covergroup g;
    cp: coverpoint x;
    cq: coverpoint y;
    xc: cross cp, cq;
  endgroup

  g a;

  initial begin
    a = new;
    x = 2'd1;
    y = 2'd2;
    #1;
    $display("before write");
`ifdef WEIGHT
    a.cp.option.weight = bad;
`elsif GOAL_HIGH
    a.option.goal = bad;
`elsif GOAL_NEG
    a.xc.option.goal = bad;
`elsif AT_LEAST
    a.cq.option.at_least = bad;
`elsif TYPE_WEIGHT
    g::cp::type_option.weight = bad;
`elsif TYPE_GOAL
    g::type_option.goal = bad;
`else
    a.option.goal = lo;
    a.cp.option.goal = hi;
    a.option.at_least = lo;
    a.xc.option.weight = big;
    g::type_option.goal = hi;
    g::type_option.weight = lo;
`endif
    $display("after write goal %0d %0d at_least %0d weight %0d type %0d %0d", a.option.goal,
             a.cp.option.goal, a.option.at_least, a.xc.option.weight, g::type_option.goal,
             g::type_option.weight);
    $finish;
  end

  final $display("final");
endmodule
