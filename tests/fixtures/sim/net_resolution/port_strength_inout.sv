// llg-test-fixture: tests/fixtures/sim/net_resolution/port_strength_inout.sv
// IEEE 1364-2001 3.4.2, 6.7 and 12.3.10; IEEE 1800-2009 23.3 and 28.11:
// output-port drive strengths remain part of the structural driver when the
// child port is connected to a parent net.
module source(d, out);
    input wire d;
    output out;
    wire (weak0, strong1) out;
    assign out = d;
endmodule

module tb;
    logic d;
    wire net;
    source s(.d(d), .out(net));
    assign (pull0, pull1) net = ~d;

    initial begin
        d = 0;
        #1 $display("low=%b", net);
        d = 1;
        #1 $display("high=%b", net);
        d = 1'bz;
        #1 $display("released=%b", net);
        $finish(0);
    end
endmodule
