// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_instance_event.sv
// SIM-044 negative: a sampling event that reads a property of the owning object needs one waiter per instance, which llg does not lower yet; it is rejected at the event (IEEE 1800-2009 19.4).
module tb;
  class C;
    bit tick;
    bit [1:0] level;
    covergroup g @(posedge tick);
      coverpoint level;
    endgroup
    function new();
      g = new;
    endfunction
  endclass
  C o = new;
  initial $finish;
endmodule
