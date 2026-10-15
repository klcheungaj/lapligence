// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_embedded_outside_new.sv
// SIM-044 negative: an embedded covergroup may only be constructed in its class's new (IEEE 1800-2009 19.4).
module tb;
  class C;
    bit [1:0] p;
    covergroup g;
      coverpoint p;
    endgroup
    function new();
    endfunction
    function void make();
      g = new;
    endfunction
  endclass
  C o = new;
  initial begin
    o.make();
    $finish;
  end
endmodule
