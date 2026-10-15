// llg-test-fixture: SIM-042 A02 system tasks and functions. systf.c registers
// sized, integer, real and time functions and tasks with compiletf, sizetf
// and calltf (some NULL), iterates vpiArgument, keeps per-call user data and
// checks the return conventions. Run with the prototypes in sim_042.rs.
// Expected output: systf.out, derived in readme.md.
`timescale 1ns/1ns
module tb;
    logic [7:0] v = 8'd5;
    logic [7:0] mem [0:1];
    real r = 0.5;
    logic signed [11:0] total;
    int none;
    real half;
    time big;
    initial begin : run
        mem[1] = 8'h3;
        total = $sum(v, 12'sd10);
        $display("sum=%0d", total);
        total = $sum(v, -12'sd7);
        $display("sum=%0d", total);
        none = $noret();
        $display("noret=%0d", none);
        half = $half(r);
        $display("half=%0.2f", half);
        big = $big();
        $display("big=%0d", big);
        $noop;
        $args(v, mem[1], v[3:0], 3'b1x0, "txt", r, v + 8'd1);
        $display("v=%0d mem1=%0d", v, mem[1]);
        for (int k = 0; k < 2; k++) $count;
        $count;
        $late_put;
        $finish;
    end
endmodule
