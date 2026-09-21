// llg-test-fixture: tests/fixtures/sim/partial_features/udp_sequential_rejected.sv
// IEEE 1364-2001 8.3; IEEE 1800-2009 29.5. Sequential UDPs remain outside
// the combinational UDP simulator profile.
primitive udp_latch(out, enable, data);
    output reg out;
    input enable, data;
    table
        0 0 : ? : 0;
        0 1 : ? : 1;
    endtable
endprimitive

module tb;
    reg enable, data;
    wire out;
    udp_latch u_seq(out, enable, data);
    initial begin
        enable = 0;
        data = 0;
        $finish(0);
    end
endmodule
