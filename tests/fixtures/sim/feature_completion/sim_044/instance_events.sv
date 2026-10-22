// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/instance_events.sv
// SIM-044: sampling events on one instance's storage: an owner property (a virtual interface), a ref argument and an input class-handle argument; each instance waits on its own event, independently of the process that constructed it (IEEE 1800-2009 19.3, 19.4).
interface bus_if;
  logic clk = 0;
  logic [1:0] data = 0;
endinterface
class Source;
  logic clk = 0;
  bit [1:0] val;
endclass
class Monitor;
  virtual bus_if vif;
  covergroup cg @(posedge vif.clk); option.per_instance = 1;
    coverpoint vif.data;
  endgroup
  function new(virtual bus_if v);
    vif = v;
    cg = new;
  endfunction
endclass
module tb;
  bus_if b0();
  bus_if b1();
  logic ca = 0, cb = 0;
  bit [1:0] va, vb;
  covergroup by_ref(ref logic ck, ref bit [1:0] v) @(posedge ck); option.per_instance = 1;
    coverpoint v;
  endgroup
  covergroup by_handle(Source s) @(posedge s.clk); option.per_instance = 1;
    coverpoint s.val;
  endgroup
  Source src = new;
  by_ref ra = new(ca, va);
  by_ref rb = new(cb, vb);
  by_handle h = new(src);
  Monitor m0, m1;
  initial begin
    m0 = new(b0);
    m1 = new(b1);
    wait fork;
    $display("wait fork returned at %0t", $time);
    repeat (3) begin
      #1 b0.clk = 1; ca = 1; src.clk = 1;
      #1 b0.clk = 0; ca = 0; src.clk = 0;
      b0.data++; va++; src.val += 2;
    end
    #1 b1.clk = 1; cb = 1;
    #1 $display("m0=%0.2f m1=%0.2f ra=%0.2f rb=%0.2f h=%0.2f",
                m0.cg.get_inst_coverage(), m1.cg.get_inst_coverage(),
                ra.get_inst_coverage(), rb.get_inst_coverage(), h.get_inst_coverage());
    m0.cg.stop();
    #1 b0.clk = 1;
    #1 b0.clk = 0;
    $display("stopped m0=%0.2f", m0.cg.get_inst_coverage());
    m0.cg.start();
    #1 b0.clk = 1;
    #1 $display("started m0=%0.2f type=%0.2f", m0.cg.get_inst_coverage(), m0.cg.get_coverage());
    $finish;
  end
endmodule
