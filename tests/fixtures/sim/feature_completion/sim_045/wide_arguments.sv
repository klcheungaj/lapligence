// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/wide_arguments.sv
// SIM-045: a 71-bit coverpoint with explicit bins and a transition bin that
// reads a covergroup argument (IEEE 1800-2009 19.5, 19.5.1).
module tb;
  bit clk = 0;
  bit [70:0] x;
  bit [3:0] y;
  int lo = 2;
  covergroup cg(int low) @(posedge clk);
    option.per_instance = 1;
    coverpoint x { bins zero = {0}; bins big = {71'h1 << 70}; }
    coverpoint y { bins lowv = {[low:3]}; bins tr = (low => 3); }
  endgroup
  cg c = new(lo);
  initial begin
    repeat (3) begin
      #1 clk = 1; #1 clk = 0;
      x = 71'h1 << 70; y = 2;
    end
    #1 clk = 1; #1 clk = 0;
    y = 3;
    #1 clk = 1; #1 clk = 0;
    $display("inst %0.2f type %0.2f x %0.2f y %0.2f", c.get_inst_coverage(), c.get_coverage(), c.x.get_inst_coverage(), c.y.get_inst_coverage());
    $display("done");
    $finish;
  end
endmodule
