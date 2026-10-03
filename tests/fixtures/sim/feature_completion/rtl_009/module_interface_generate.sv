// SV 23.3.3.5, 25.5: interface modport arrays, generate, instance arrays.
interface bus #(parameter int N = 2);
    logic [7:0] d[N];
    logic [7:0] s;
    modport src(output d);
    modport dst(input d, output s);
endinterface

module prod(bus.src b, input logic [7:0] base);
    for (genvar i = 0; i < 2; i++) begin : g
        assign b.d[i] = base + 8'(i);
    end
endmodule

module cons(bus.dst b);
    assign b.s = b.d[0] + b.d[1];
endmodule

module leaf(input logic [7:0] v[2], output logic [7:0] w);
    assign w = v[1] - v[0];
endmodule

module mid(bus.dst b, output logic [7:0] w);
    leaf l(.v(b.d), .w(w));
endmodule

module cel(input logic [3:0] a, output logic [3:0] y,
           input logic [1:0] p[2], output logic [1:0] q[2]);
    assign y = ~a;
    assign q[0] = p[1];
    assign q[1] = p[0];
endmodule

module tb;
    bus #(2) bb();
    logic [7:0] base = 8'd5;
    logic [7:0] w[2], wm;
    logic [15:0] av;
    wire [15:0] yv;
    logic [1:0] pa[3:0][2];
    logic [1:0] qa[0:3][2];
    prod p(bb, base);
    cons c(bb);
    mid m(bb, wm);
    for (genvar j = 0; j < 2; j++) begin : gl
        leaf l(.v(bb.d), .w(w[j]));
    end
    cel u[3:0](.a(av), .y(yv), .p(pa), .q(qa));
    initial begin
        av = 16'h1234;
        for (int i = 0; i < 4; i++) begin
            pa[i][0] = 2'(i);
            pa[i][1] = 2'(3 - i);
        end
        #1 $display("%0d %0d %0d %0d", bb.s, w[0], w[1], wm);
        $display("%h", yv);
        for (int i = 0; i < 4; i++) $write("%0d%0d ", qa[i][0], qa[i][1]);
        $display;
        base = 8'd100;
        #1 $display("%0d %0d %0d", bb.s, bb.d[0], bb.d[1]);
        $finish(0);
    end
endmodule
