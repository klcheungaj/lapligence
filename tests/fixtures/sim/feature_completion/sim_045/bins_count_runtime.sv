// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/bins_count_runtime.sv
// SIM-045: a bins array whose size argument is not positive is a run-time
// error when the instance is constructed, and it ends the simulation; the
// final procedures run and the run exits with a failure status (decision
// S45-D5). A positive size gives that many bins.
module tb;
  bit [3:0] a;
  covergroup g_count(int n); option.per_instance = 1;
    cp: coverpoint a { bins b[n] = {[0:3]}; bins other = {9}; }
  endgroup
  g_count two;
  g_count none;
  final $display("final %0.2f", two.get_inst_coverage());
  initial begin
    two = new(2);
    a = 9; two.sample();
    $display("two %0.2f", two.get_inst_coverage());
    none = new(0);
    $display("after new(0)");
    $finish;
  end
endmodule
