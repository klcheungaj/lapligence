// llg-test-fixture: tests/fixtures/sim/partial_features/net_declaration_transition.sv
module tb;
    timeunit 1ns;
    timeprecision 1ps;
    logic source;
    wire #(1, 3, 5) tuple_net;
    assign tuple_net = source;
    wire #1.5 real_net;
    assign real_net = source;

    initial begin
        source = 1'b0;
        #1 $strobe("t1 %b %b", tuple_net, real_net);
        #1 source = 1'b1;
        #1 $strobe("t3 %b %b", tuple_net, real_net);
        #1 source = 1'b0;
        #1 $strobe("t5 %b %b", tuple_net, real_net);
        #2 $strobe("t7 %b %b", tuple_net, real_net);
        #1 $finish(0);
    end
endmodule
