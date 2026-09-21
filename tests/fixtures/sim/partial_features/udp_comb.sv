// llg-test-fixture: tests/fixtures/sim/partial_features/udp_comb.sv
// IEEE 1364-2001 8.1-8.2, 8.6; IEEE 1800-2009 29.3-29.4, 29.8.
// Combinational UDP rows use 0/1/x, b, and ? symbols. Runtime Z matches x.
primitive udp_xor(y, a, b);
    output y;
    input a, b;
    table
        0 0 : 0;
        0 1 : 1;
        1 0 : 1;
        1 1 : 0;
        x 0 : 1;
        b x : x;
        ? x : x;
    endtable
endprimitive

module tb;
    reg a, b, c, d;
    reg [1:0] va, vb;
    wire y;
    wire delayed_y;
    wire [1:0] array_y;
    wire resolved;

    udp_xor scalar(y, a, b);
    udp_xor (strong0, strong1) #1 delayed(delayed_y, a, b);
    udp_xor array[1:0](array_y, va, vb);
    udp_xor driver0(resolved, c, d);
    udp_xor driver1(resolved, a, b);

    initial begin
        a = 0; b = 0;
        #1 $display("known %b", y);
        a = 0; b = 1;
        #1 $display("known %b", y);
        a = 1; b = 0;
        #1 $display("known %b", y);
        a = 1; b = 1;
        #1 $display("known %b", y);

        a = 1'bx; b = 0;
        #1 $display("x %b", y);
        a = 1'bz; b = 0;
        #1 $display("z %b", y);
        a = 1'bx; b = 1;
        #1 $display("unmatched %b", y);
        a = 0; b = 1'bx;
        #1 $display("b_symbol %b", y);
        a = 1'bx; b = 1'bx;
        #1 $display("wildcard %b", y);

        va = 2'b01; vb = 2'b11;
        #1 $display("array %b", array_y);

        a = 0; b = 0; c = 0; d = 0;
        #1 $display("resolved %b", resolved);
        c = 0; d = 1;
        #1 $display("resolved %b", resolved);
        a = 1'bz; b = 0; c = 0; d = 0;
        #1 $display("resolved_z %b", resolved);
        a = 0; b = 0;
        #0 $display("delay_before %b", delayed_y);
        #1 $display("delay_after %b", delayed_y);
        $finish(0);
    end
endmodule
