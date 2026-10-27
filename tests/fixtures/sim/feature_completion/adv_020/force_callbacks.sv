// llg-test-fixture: ADV-020 force and release callbacks. force_callbacks.c
// ($watch, $vpi_force, $stale) registers cbForce and cbRelease records, for
// every statement and on objects, and cbAssign/cbDeassign records, then
// reports each statement with its vpiForce/vpiRelease handle, vpiLhs and
// left-hand value (IEEE 1800-2009 38.36.1). Expected output:
// force_callbacks.out.
`timescale 1ns/1ns
module tb;
    parameter int P = 1;
    logic [7:0] v, w;
    wire [7:0] n;
    real r;
    wire [3:0] na [0:2];
    logic [3:0] a, b;

    assign n = w;
    assign na[0] = 4'h0;
    assign na[1] = 4'h1;
    assign na[2] = 4'h2;

    initial begin
        v = 8'h11;
        w = 8'h22;
        r = 1.5;
        a = 0; b = 0;
        $watch;
        #1 force v = 8'h5a;
        #1 release v;
        #1 force n = 8'hf0;
        #1 force n[3:0] = 4'h3;
        #1 release n;
        #1 force r = 2.5;
        #1 release r;
        #1 force na[1] = 4'h9;
        #1 release na[1];
        #1 force {a, b} = 8'hc3;
        #1 release {a, b};
        #1 force w = v + 8'd1;
        #1 v = 8'h40;
        #1 release w;
        #1 $vpi_force;
        #1 $stale;
        #1 force v = 8'h01;
        #1 release v;
        $display("v=%h w=%h n=%h r=%g na[1]=%h a=%h b=%h", v, w, n, r, na[1], a, b);
        $finish;
    end
endmodule
