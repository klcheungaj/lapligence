// llg-test-fixture: tests/fixtures/sim/review_bundle/r12_udp_ansi.sv
// SV 29.3/29.4: an ANSI UDP declaration uses declared scalar ports and a
// combinational table. These three inputs distinguish a known 0, 1, then 0.
primitive udp_ansi_and(output y, input a, b);
    table
        0 ? : 0;
        ? 0 : 0;
        1 1 : 1;
    endtable
endprimitive

module tb;
    reg a, b;
    wire y;
    udp_ansi_and dut(y, a, b);
    initial begin
        a = 0; b = 1;
        #1;
        if (y !== 1'b0) begin $display("FAIL first"); $finish(1); end
        a = 1; b = 1;
        #1;
        if (y !== 1'b1) begin $display("FAIL second"); $finish(1); end
        a = 1; b = 0;
        #1;
        if (y !== 1'b0) begin $display("FAIL third"); $finish(1); end
        $display("udp_ansi=0,1,0");
        $finish(0);
    end
endmodule
