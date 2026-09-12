// llg-test-fixture: tests/fixtures/sim/partial_features/net_declaration_overflow.sv
module tb;
    logic source;
    wire #(1.0e300) bad = source;
    initial $finish;
endmodule
