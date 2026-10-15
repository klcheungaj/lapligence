// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/strobe.sv
// SIM-044: type_option.strobe samples once per time slot in the Postponed region, after the slot's nonblocking updates, for registered and per-instance events; a sample() call is not delayed (IEEE 1800-2009 19.3, 19.7.1).
module tb;
  logic clk = 0;
  bit [2:0] v;
  covergroup at_event @(posedge clk);
    option.at_least = 2;
    lo: coverpoint v { bins b = {[0:3]}; }
    hi: coverpoint v { bins b = {[4:7]}; }
  endgroup
  covergroup strobed @(posedge clk);
    type_option.strobe = 1;
    lo: coverpoint v { bins b = {[0:3]}; }
    hi: coverpoint v { bins b = {[4:7]}; }
  endgroup
  covergroup strobed_twice @(posedge clk);
    type_option.strobe = 1;
    option.at_least = 2;
    coverpoint v { bins hi = {[4:7]}; }
  endgroup
  covergroup called;
    type_option.strobe = 1;
    coverpoint v { bins lo = {[0:3]}; }
  endgroup
  class Box;
    logic c = 0;
    bit [2:0] x;
    covergroup cg @(posedge c);
      type_option.strobe = 1;
      four: coverpoint x { bins b = {4}; }
      six: coverpoint x { bins b = {6}; }
    endgroup
    function new();
      cg = new;
    endfunction
  endclass
  at_event e = new;
  strobed s = new;
  strobed_twice t = new;
  called p = new;
  Box b = new;
  always @(posedge clk) v <= v + 3'd4;
  initial begin
    // Two rising edges in one slot, both before the slot's nonblocking
    // update of v from 1 to 5.
    #1 v = 1; clk = 1;
    #0 clk = 0;
    #0 clk = 1;
    p.sample();
    $display("called=%0.2f", p.get_inst_coverage());
    b.x = 4; b.c = 1;
    #0 b.x = 6;
    #1 $display("event lo=%0.2f hi=%0.2f", e.lo.get_inst_coverage(), e.hi.get_inst_coverage());
    $display("strobe lo=%0.2f hi=%0.2f twice=%0.2f", s.lo.get_inst_coverage(), s.hi.get_inst_coverage(), t.get_inst_coverage());
    $display("box four=%0.2f six=%0.2f", b.cg.four.get_inst_coverage(), b.cg.six.get_inst_coverage());
    $finish;
  end
endmodule
