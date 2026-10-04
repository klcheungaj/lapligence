// IEEE 1800-2009 9.2.2.2-9.2.2.4 and 6.5: each always_comb/always_latch/
// always_ff writer owns the longest static prefix it assigns. Disjoint nested
// record members, packed members, packed ranges and constant array cells are
// separate storage, so every process below is the sole writer of its part.
module tb;
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pk_t;
    typedef struct { logic [7:0] a; pk_t p; logic [7:0] arr [0:3]; } in_t;
    typedef struct { in_t inner; logic [7:0] b; } out_t;
    out_t s;
    pk_t pk;
    logic [7:0] x, y, z;
    logic [7:0] v;
    logic [7:0] m [0:3];
    logic clk, en;

    always_comb s.inner.a = x;
    always_comb s.inner.p.hi = y[3:0];
    always_latch if (en) s.inner.p.lo = y[7:4];
    always_comb s.inner.arr[1] = z;
    always_ff @(posedge clk) s.inner.arr[2] <= x ^ z;
    always_comb s.b = s.inner.a + s.inner.arr[1];
    always_comb pk.hi = x[3:0];
    always_ff @(posedge clk) pk.lo <= z[3:0];
    always_comb v[3:0] = x[7:4];
    always_latch if (en) v[7:4] = z[7:4];
    always_ff @(posedge clk) m[0] <= x;
    always_comb m[1] = y;
    always_latch if (en) m[2] = z;
    assign m[3] = x + y;

    initial begin
        clk = 0;
        en = 1;
        x = 8'h12;
        y = 8'h34;
        z = 8'h56;
        #1 clk = 1;
        #1 $display("t2 %h %h %h %h %h %h %h | %h %h %h %h", s.inner.a, s.inner.p,
                    s.inner.arr[1], s.inner.arr[2], s.b, pk, v, m[0], m[1], m[2], m[3]);
        clk = 0;
        en = 0;
        x = 8'hf0;
        y = 8'h78;
        z = 8'h0f;
        #1 $display("t3 %h %h %h %h %h %h %h | %h %h %h %h", s.inner.a, s.inner.p,
                    s.inner.arr[1], s.inner.arr[2], s.b, pk, v, m[0], m[1], m[2], m[3]);
        clk = 1;
        #1 $display("t4 %h %h %h %h %h %h %h | %h %h %h %h %h", s.inner.a, s.inner.p,
                    s.inner.arr[1], s.inner.arr[2], s.b, pk, v, m[0], m[1], m[2], m[3],
                    s.inner.arr[0]);
        $finish(0);
    end
endmodule
