// llg-test-fixture: tests/fixtures/sim/partial_features/reference_resizable_rejected.sv
// LRM: IEEE 1800-2009 23.2.2.2 and 23.3.3
module dynamic_leaf(ref logic [3:0] value[]);
endmodule

module tb;
    logic [3:0] value[];
    dynamic_leaf c(value);
endmodule
