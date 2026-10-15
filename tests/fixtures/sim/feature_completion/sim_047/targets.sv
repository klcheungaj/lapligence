// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/targets.sv
// SIM-047 step 2: $dumpvars levels, scopes, variables and whole arrays
// (IEEE 1364-2001 18.1.2, IEEE 1800-2009 21.7.1.2). The calls of one time
// step add up; a call at a later time is ignored with a warning (S47-D3).
`timescale 1ns/1ns
module leaf (input logic [1:0] i);
    logic [1:0] r;
    assign r = ~i;
endmodule

module mid (input logic [1:0] i);
    logic [1:0] m;
    assign m = i;
    leaf l(.i(m));
endmodule

module tb;
    logic [1:0] a;
    logic [1:0] b;
    logic [7:0] mem [0:1];
    mid u1(.i(a));
    mid u2(.i(b));
    for (genvar k = 0; k < 2; k++) begin : g
        logic [1:0] gv;
        assign gv = a + 2'(k);
        leaf l(.i(gv));
    end

    initial begin
        a = 2'd0;
        b = 2'd3;
        mem[0] = 8'h10;
        mem[1] = 8'h11;
        $dumpfile("trace.vcd");
        $dumpvars(1, tb.u1);
        $dumpvars(0, tb.g[1].l);
        $dumpvars(0, tb.mem, tb.b);
        $strobe("%0t a=%b b=%b u1.m=%b u1.l.r=%b g0=%b,%b g1=%b,%b", $time, a, b,
                u1.m, u1.l.r, g[0].gv, g[0].l.r, g[1].gv, g[1].l.r);
        #1 a = 2'd1;
        b = 2'd2;
        mem[1] = 8'h22;
        $dumpvars(0, tb);
        $strobe("%0t a=%b b=%b u1.m=%b u1.l.r=%b g0=%b,%b g1=%b,%b", $time, a, b,
                u1.m, u1.l.r, g[0].gv, g[0].l.r, g[1].gv, g[1].l.r);
        #1 $finish;
    end
endmodule
