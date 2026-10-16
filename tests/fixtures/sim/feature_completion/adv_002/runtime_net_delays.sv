// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/runtime_net_delays.sv
// IEEE 1800-2009 28.16, 28.16.1, 10.3.3. A runtime-valued net delay applies
// the value current when the net changes (S2-D9), per bit on a vector net
// (S2-D3).
`timescale 1ns/1ns
module tb;
    reg a;
    reg [1:0] v;
    integer dn, dr, df;
    wire #(dn) n;
    wire [1:0] #(dr, df) nv;
    assign n = a;
    assign nv = v;
    initial begin
        $monitor("%0t n=%b nv=%b", $time, n, nv);
        dn = 2; dr = 5; df = 3;
        a = 0; v = 2'b01;
        #10 a = 1; v = 2'b10;
        #10 dn = 6; df = 1;
        #10 a = 0; v = 2'b01;
        #10 dn = 1;
        #1 a = 1;
        #10 $finish(0);
    end
endmodule
