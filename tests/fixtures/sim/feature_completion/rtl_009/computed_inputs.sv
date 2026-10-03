// SV 23.3.3.2, 23.3.3.5, 7.6: computed fixed-array inputs, reversed bounds.
module child(input logic [7:0] a[0:3], input logic [7:0] b[3:0],
             output logic [7:0] s[2]);
    always_comb begin
        s[0] = a[0] + b[0];
        s[1] = a[3] + b[3];
    end
endmodule

module row_sum(input logic [7:0] r[2], output logic [7:0] y);
    assign y = r[0] + r[1];
endmodule

module tb;
    typedef logic [7:0] row_t[2];
    logic [7:0] m[4:1];
    logic [7:0] m2[0:3];
    logic [7:0] n[0:3];
    logic [7:0] g[0:2][2];
    logic [7:0] r0[2], r1[1:0], r2[2];
    logic [7:0] y0, y1;
    int k = 0;
    bit sel = 0;
    function automatic logic [7:0] f(int v);
        return 8'(v * 3);
    endfunction
    child c0(.a(m), .b(n), .s(r0));
    child c1(.a('{f(1), f(2), 8'hx, 8'h4}), .b(n[0:3]), .s(r1));
    child c2(.a(sel ? m : m2), .b(n), .s(r2));
    row_sum s0(.r(g[k]), .y(y0));
    row_sum s1(.r(row_t'(g[2])), .y(y1));
    initial begin
        m[4] = 8'd1; m[3] = 8'd2; m[2] = 8'd3; m[1] = 8'd4;
        m2 = '{4{8'd1}};
        n[0] = 8'd10; n[1] = 8'd20; n[2] = 8'd30; n[3] = 8'd40;
        g[0] = '{8'd1, 8'd2};
        g[1] = '{8'd10, 8'd20};
        g[2] = '{8'd7, 8'd9};
        #1 $display("%0d %0d | %0d %0d | %0d %0d | %0d %0d",
                    r0[0], r0[1], r1[0], r1[1], r2[0], r2[1], y0, y1);
        m[1] = 8'hz;
        sel = 1;
        k = 1;
        #1 $display("%h %h | %0d %h | %0d", r0[0], r0[1], r2[0], r2[1], y0);
        g[1][0] = 8'd100;
        #1 $display("%0d", y0);
        $finish(0);
    end
endmodule
