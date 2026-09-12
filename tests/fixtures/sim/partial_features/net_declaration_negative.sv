// llg-test-fixture: tests/fixtures/sim/partial_features/net_declaration_negative.sv
module tb;
    logic source;
    wire #(-1.0) bad = source;
    initial $finish;
endmodule
