// SV 23.3.3.2, 23.3.3.5, 6.24.1: 65,537-cell fixed arrays cross ports
// through descriptor copies: whole arrays, runtime rows, converting casts,
// conditionals, reversed bounds and ref sharing.
typedef bit [16:0] b17_t[0:65536];
typedef logic [16:0] l17_t[0:65536];

module pass(input logic [16:0] a[0:65536], output logic [16:0] b[0:65536]);
    always_comb b = a;
endmodule

module refw(ref logic [16:0] r[0:65536]);
    initial #1 r[65536] = 17'h1abcd;
endmodule

module tb;
    logic [16:0] x[0:65536], z[0:65536];
    logic [16:0] big[2][0:65536];
    logic [16:0] y[0:65536], o1[0:65536], o2[0:65536], o3[0:65536], o4[65536:0];
    int sel = 0;
    bit c = 0;
    pass p0(.a(x), .b(y));
    pass p1(.a(big[sel]), .b(o1));
    pass p2(.a(l17_t'(b17_t'(x))), .b(o2));
    pass p3(.a(c ? x : z), .b(o3));
    pass p4(.a(x), .b(o4));
    refw w(.r(z));
    initial begin
        x[0] = 17'h1;
        x[5] = 17'h1x0z0;
        x[65536] = 17'h1ffff;
        big[0][3] = 17'h3;
        big[1][3] = 17'h13;
        #2;
        $display("%h %h %h %h", y[0], y[5], y[65536], y[7]);
        $display("%h %h", o1[3], o1[4]);
        $display("%h %h %h", o2[5], o2[7], o2[65536]);
        $display("%h %h", o3[0], o3[65536]);
        $display("%h %h", o4[65536], o4[0]);
        sel = 1;
        c = 1;
        x[1] = 17'h2;
        #1 $display("%h %h %h %h", o1[3], o3[1], y[1], o2[1]);
        $finish(0);
    end
endmodule
