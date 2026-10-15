// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/dropped_waiters.sv
// SIM-044: an instance whose event reads its owner's storage keeps waiting and sampling after the program drops every handle to the instance and to its owner, also with a collection at every allocation (IEEE 1800-2009 19.3; decisions S44-D3, S44-D5).
class Source;
  logic clk = 0;
  bit [1:0] val;
endclass
class Monitor;
  Source src;
  covergroup cg @(posedge src.clk);
    coverpoint src.val;
  endgroup
  function new(Source s);
    src = s;
    cg = new;
  endfunction
endclass
class Feeder;
  Source src;
  function new(Source s);
    src = s;
  endfunction
  task drive(bit [1:0] v);
    #1 src.val = v; src.clk = 1;
    #1 src.clk = 0;
  endtask
endclass
module tb;
  Monitor m, keep;
  Feeder f;
  int k;
  initial begin
    begin
      Source s;
      s = new;
      f = new(s);
      keep = new(s);
    end
    for (k = 0; k < 4; k++) begin
      m = new(f.src);
      m = null;
      f.drive(k[1:0]);
    end
    $display("keep=%0.2f type=%0.2f", keep.cg.get_inst_coverage(), keep.cg.get_coverage());
    $finish;
  end
endmodule
