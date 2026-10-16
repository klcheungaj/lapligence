// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/net_array_ports.sv
// IEEE 1800-2009 10.3.3, 10.11, 23.3.3.7, 28.16. The net delay of a
// net-array declaration also applies to the element reached through an
// inout port (the parent's declaration wins the same-type collapse) and to
// an alias of one element: the alias names the same delayed net, so the
// delay is applied once.
`timescale 1ns/1ns
module child (inout wire [1:0] io [0:1], input wire en, input wire [1:0] d);
    assign io[1] = en ? d : 2'bzz;
endmodule

module tb;
    reg en;
    reg [1:0] d;
    wire [1:0] #3 bus [0:1];
    wire [1:0] al;
    child c (.io(bus), .en(en), .d(d));
    alias al = bus[0];
    assign bus[0] = d;

    initial begin
        $monitor("%0t bus0=%b bus1=%b al=%b", $time, bus[0], bus[1], al);
        en = 1'b1;
        d = 2'b01;
        #10 d = 2'b10;
        #10 en = 1'b0;
        #10 $finish(0);
    end
endmodule
