// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/block_returns.sv
// SIM-044: an `end` block event samples after a subroutine (also a task that waits) that leaves through `return`, after its return value is set; named-block events sample before the first and after the last statement, and not after a disable or a jump out of the block (IEEE 1800-2009 19.3; decisions S44-D7, S44-D8).
module tb;
  int n;
  int r;
  int k;
  function automatic int f(int a);
    n = a;
    if (a > 1) return a * 10;
    n = a + 100;
    return 1;
  endfunction
  task automatic t(int a);
    n = a;
    if (a == 5) return;
    n = 7;
  endtask
  task automatic pause(int a);
    n = a;
    #1;
    if (a == 3) return;
    n = 9;
  endtask
  covergroup at_pause @@(end pause);
    coverpoint n { bins three = {3}; bins nine = {9}; }
  endgroup
  covergroup at_f @@(end f);
    coverpoint n { bins two = {2}; bins hundred = {100}; bins never = {0}; }
  endgroup
  covergroup at_t @@(end t);
    coverpoint n { bins five = {5}; bins seven = {7}; }
  endgroup
  covergroup enter @@(begin blk);
    coverpoint n { bins zero = {0}; bins one = {1}; bins two = {2}; }
  endgroup
  covergroup leave @@(end blk);
    coverpoint n { bins thirty = {30}; bins disabled = {21}; bins jumped = {22}; }
  endgroup
  at_f gf = new;
  at_t gt = new;
  enter ge = new;
  leave gl = new;
  at_pause gp = new;
  initial begin
    r = f(2);
    $display("r=%0d f=%0.2f", r, gf.get_inst_coverage());
    r = f(0);
    $display("r=%0d f=%0.2f", r, gf.get_inst_coverage());
    t(5);
    $display("t=%0.2f", gt.get_inst_coverage());
    t(6);
    $display("t=%0.2f", gt.get_inst_coverage());
    pause(3);
    $display("pause=%0.2f at %0t", gp.get_inst_coverage(), $time);
    for (k = 0; k < 3; k++) begin
      n = k;
      begin : blk
        n = 20 + k;
        if (k == 1) disable blk;
        if (k == 2) break;
        n = 30 + k;
      end
    end
    $display("enter=%0.2f leave=%0.2f", ge.get_inst_coverage(), gl.get_inst_coverage());
    $finish;
  end
endmodule
