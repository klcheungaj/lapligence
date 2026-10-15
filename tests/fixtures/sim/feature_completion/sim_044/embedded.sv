// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/embedded.sv
// SIM-044: covergroups embedded in a class are constructed by the class constructor and sample the object's properties; an event-sampled group, its owner and handle and string arguments stay alive after the last user reference is dropped (IEEE 1800-2009 19.4, 19.3).
module tb;
  logic clk = 0;
  class Junk;
    int payload[16];
  endclass
  class Item;
    bit [1:0] p;
    function new(bit [1:0] v);
      p = v;
    endfunction
  endclass
  class Agent;
    bit [3:0] level;
    covergroup on_clock @(posedge clk);
      coverpoint level;
    endgroup
    covergroup on_call with function sample(bit [1:0] k);
      ck: coverpoint k;
      cl: coverpoint level { bins low = {[0:7]}; bins high = {[8:15]}; }
    endgroup
    function new(bit [3:0] start);
      level = start;
      on_clock = new;
      on_call = new;
    endfunction
    function void report(bit [1:0] k);
      on_call.sample(k);
    endfunction
  endclass
  covergroup held(Item item, string tag);
    coverpoint item.p;
  endgroup
  Agent a1, a2;
  held h;
  Item it;
  Junk junk;
  initial begin
    a1 = new(3);
    a2 = new(12);
    a1.report(1);
    a1.report(2);
    a2.report(1);
    $display("call a1=%0.2f a2=%0.2f", a1.on_call.get_inst_coverage(),
             a2.on_call.get_inst_coverage());
    #1 clk = 1;
    #1 clk = 0;
    a1.level = 4;
    a1 = null;
    for (int i = 0; i < 50000; i++) junk = new;
    #1 clk = 1;
    #1 clk = 0;
    $display("clock a2=%0.2f total=%0.4f", a2.on_clock.get_inst_coverage(), $get_coverage());
    it = new(2);
    h = new(it, "first");
    it = null;
    for (int i = 0; i < 50000; i++) junk = new;
    h.sample();
    $display("held=%0.2f", h.get_inst_coverage());
    $finish;
  end
endmodule
