// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/sampling.sv
// SIM-044: a clocking event samples the values before the slot's nonblocking updates; with function sample() samples its arguments; @@(begin/end f) samples before the first and after the last statement, and not after a disable (IEEE 1800-2009 19.3, 19.8.1).
module tb;
  logic clk = 0;
  bit [1:0] d;
  int calls;
  covergroup ev @(posedge clk);
    coverpoint d;
  endgroup
  covergroup ws with function sample(bit [1:0] x);
    coverpoint x;
  endgroup
  function automatic void step(int k);
    calls = k;
  endfunction
  task automatic halt(int k);
    calls = k;
    if (k == 9) disable halt;
    calls = k + 1;
  endtask
  covergroup at_begin @@(begin step);
    coverpoint calls { bins zero = {0}; bins one = {1}; }
  endgroup
  covergroup at_end @@(end step or end halt);
    coverpoint calls { bins zero = {0}; bins one = {1}; bins nine = {9}; bins ten = {10}; bins five = {5}; }
  endgroup
  ev e = new;
  ws w = new;
  at_begin b = new;
  at_end f = new;
  always @(posedge clk) d <= d + 1;
  initial begin
    #1 clk = 1;
    #1 clk = 0;
    #1 clk = 1;
    #1 clk = 0;
    $display("event=%0.2f d=%0d", e.get_inst_coverage(), d);
    d = 1;
    w.sample(d);
    d = 3;
    $display("with-sample=%0.2f", w.get_inst_coverage());
    w.sample(2'd2);
    $display("with-sample=%0.2f", w.get_inst_coverage());
    step(1);
    $display("begin=%0.2f end=%0.2f", b.get_inst_coverage(), f.get_inst_coverage());
    halt(9);
    $display("after disable end=%0.2f calls=%0d", f.get_inst_coverage(), calls);
    halt(4);
    $display("end=%0.2f calls=%0d", f.get_inst_coverage(), calls);
    $finish;
  end
endmodule
