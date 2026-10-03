// IEEE 1800-2009 6.5, 7.4 and 10.9: pattern leaves select bits of net-array
// cells, name a whole net array, or name a true alias of one cell. Every
// leaf keeps its own contribution next to independent drivers.
module tb;
    typedef logic [1:0][1:0] bits_t;
    typedef logic [3:0] row_t[0:1];
    typedef struct { row_t n; logic [3:0] m; } whole_t;
    wire [3:0] n[0:1];
    wire [3:0] w[0:1];
    wire [3:0] m;
    wire [3:0] q[0:1];
    wire [3:0] v, u;
    alias v = q[1];
    bits_t sel = 4'b1001;
    whole_t src = '{'{4'h1, 4'h2}, 4'h3};
    logic [3:0] k = 4'hz;
    logic [1:0][3:0] p = 8'h5a;
    assign '{n[0][1:0], n[1][3:2]} = sel;
    assign n[1] = k;
    assign '{w, m} = src;
    assign '{v, u} = p;
    initial begin
        #1 $display("%b %b %h %h %h %h %h %h", n[0], n[1], w[0], w[1], m, q[0], q[1], u);
        k = 4'b0110;
        #1 $display("%b %b %h %h %h %h %h %h", n[0], n[1], w[0], w[1], m, q[0], q[1], u);
        sel = 4'b0110;
        #1 $display("%b %b %h %h %h %h %h %h", n[0], n[1], w[0], w[1], m, q[0], q[1], u);
        src.n[1] = 4'hz;
        src.m = 4'h7;
        p = 8'hc3;
        #1 $display("%b %b %h %h %h %h %h %h", n[0], n[1], w[0], w[1], m, q[0], q[1], u);
        $finish(0);
    end
endmodule
