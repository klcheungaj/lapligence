// llg-test-fixture: tests/fixtures/sim/partial_features/net_declaration_propagation.sv
`timescale 1ns/1ps
module tb;
    logic source, other;
    wire #2 declared_init = source;
    wire #2 declared_assign;
    assign declared_assign = source;
    wire driver_only;
    assign #2 driver_only = source;
    wire #2 multi_net;
    assign multi_net = source;
    assign multi_net = other;
    wire multi_driver;
    assign #2 multi_driver = source;
    assign multi_driver = other;

    initial begin
        source = 1'b0;
        other = 1'b0;
        #1 source = 1'b1;
        #1 $strobe("t2 %b %b %b %b %b", declared_init, declared_assign,
                   driver_only, multi_net, multi_driver);
        #1 $strobe("t3 %b %b %b %b %b", declared_init, declared_assign,
                   driver_only, multi_net, multi_driver);
        #1 $finish(0);
    end
endmodule
