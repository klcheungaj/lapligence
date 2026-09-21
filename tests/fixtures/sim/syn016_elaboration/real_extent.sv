// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/real_extent.sv
// IEEE 1800-2009 §7.4 requires a packed dimension expression to be integral.
module tb #(parameter real W = 1.5);
  logic [W-1:0] value;
endmodule
