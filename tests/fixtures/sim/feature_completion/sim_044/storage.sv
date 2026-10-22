// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/storage.sv
// SIM-044 storage audit: variables read only by coverpoint expressions, iff guards and a sampling event keep their storage with the optimizer on (IEEE 1800-2009 19.3, 19.5).
module tb;
  logic clk = 0;
  bit [1:0] only_point;
  bit only_guard;
  bit [3:0] only_sample;
  covergroup g @(posedge clk); option.per_instance = 1;
    coverpoint only_point iff (only_guard);
  endgroup
  covergroup w with function sample(bit [3:0] k); option.per_instance = 1;
    coverpoint k;
  endgroup
  g c = new;
  w s = new;
  initial begin
    only_guard = 1;
    only_point = 2;
    #1 clk = 1;
    #1 clk = 0;
    only_point = 3;
    only_guard = 0;
    #1 clk = 1;
    only_sample = 9;
    s.sample(only_sample);
    $display("point=%0.2f sample=%0.2f", c.get_inst_coverage(), s.get_inst_coverage());
    $finish;
  end
endmodule
