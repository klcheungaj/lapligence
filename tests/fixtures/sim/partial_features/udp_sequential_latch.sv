// llg-test-fixture: tests/fixtures/sim/partial_features/udp_sequential_latch.sv
// IEEE 1364-2001 8.3; IEEE 1800-2009 29.5. A level-sensitive sequential UDP
// (formerly the sequential rejection boundary) loads while enable is 0;
// rows that are not listed (enable 1) give x.
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
        #1 $display("load0 out=%b", out);
        data = 1;
        #1 $display("load1 out=%b", out);
        enable = 1;
        #1 $display("unlisted out=%b", out);
        $finish(0);
    end
endmodule
