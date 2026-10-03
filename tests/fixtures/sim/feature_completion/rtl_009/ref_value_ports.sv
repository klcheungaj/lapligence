// SV 23.3.3.2, 6.22.2: ref ports share the actual; value inputs are copies.
typedef struct { int n; logic [7:0] v[2]; } rec_t;

module child(ref int r, input logic [3:0] narrow,
             ref logic [7:0] ra[3], input logic [7:0] va[3],
             ref rec_t rr, input rec_t vr);
    initial begin
        #1;
        r = 37;
        ra[1] = 8'd77;
        rr.v[0] = 8'd9;
        rr.n = 6;
        // The ref formals are the parent's variables, so the parent storage
        // is already written before any update event can run.
        $display("A %0d %0d %0d %0d", tb.x, tb.a[1], tb.q.v[0], tb.q.n);
        #1 $display("B %0d %0d %0d %0d %0d", r, narrow, va[1], vr.n, vr.v[0]);
    end
endmodule

module tb;
    int x = 1;
    logic [7:0] a[3] = '{8'd1, 8'd2, 8'd3};
    rec_t q = '{n: 4, v: '{8'd0, 8'd0}};
    child c(.r(x), .narrow(x), .ra(a), .va(a), .rr(q), .vr(q));
    initial begin
        #3;
        x = 9;
        a[2] = 8'd50;
        q.n = 11;
        $display("C %0d %0d %0d", c.r, c.ra[2], c.rr.n);
        #1 $display("D %0d %0d %0d", c.narrow, c.va[2], c.vr.n);
        $finish(0);
    end
endmodule
