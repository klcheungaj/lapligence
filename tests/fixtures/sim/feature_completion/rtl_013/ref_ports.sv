// IEEE 1800-2009 23.3.3.3 and 9.2.2.2-9.2.2.4: a ref port names the actual's
// storage, through any number of module levels. always_comb in a child is
// sensitive to the referenced variable, array, record member or array cell,
// and an always_ff writing through a ref port owns only that referenced part.
typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pk_t;
typedef struct { logic [7:0] a; pk_t p; } in_t;
typedef struct { in_t inner; logic [7:0] b; } rec_t;

module leaf(ref logic [7:0] r, ref logic [7:0] arr [0:3], output logic [7:0] o);
    always_comb o = r + arr[2];
endmodule

module mid(ref logic [7:0] r, ref logic [7:0] arr [0:3], output logic [7:0] o);
    leaf u(.r(r), .arr(arr), .o(o));
endmodule

module rd1(ref logic [7:0] r, output logic [7:0] o);
    always_comb o = r + 8'd1;
endmodule

module rd2(ref logic [7:0] r, output logic [7:0] o);
    rd1 inner(.r(r), .o(o));
endmodule

module nib(ref logic [3:0] r, output logic [7:0] o);
    always_comb o = {4'h0, r} + 8'h10;
endmodule

module wr(ref logic [7:0] r, input logic c, input logic [7:0] d);
    always_ff @(posedge c) r <= d;
endmodule

module tb;
    logic [7:0] v;
    logic [7:0] a [0:3];
    logic [7:0] o_whole, o_member, o_cell, o_nib;
    rec_t s;
    logic [7:0] m [0:1];
    logic c;
    logic [7:0] d;

    mid whole(.r(v), .arr(a), .o(o_whole));
    rd2 member(.r(s.inner.a), .o(o_member));
    rd2 cell_ref(.r(m[1]), .o(o_cell));
    nib packed_member(.r(s.inner.p.lo), .o(o_nib));
    wr write_member(.r(s.b), .c(c), .d(d));
    wr write_cell(.r(m[0]), .c(c), .d(d + 8'd1));
    always_comb s.inner.p.hi = d[7:4];

    initial begin
        c = 0;
        d = 8'h30;
        v = 1;
        a[0] = 0;
        a[1] = 0;
        a[2] = 10;
        a[3] = 0;
        s.inner.a = 8'h21;
        s.inner.p.lo = 4'h7;
        m[1] = 8'h40;
        #1 $display("t1 %0d %h %h %h | %h %h %h", o_whole, o_member, o_cell, o_nib,
                    s.inner.p, s.b, m[0]);
        v = 2;
        a[1] = 99;
        #1 $display("t2 %0d %h %h %h | %h %h %h", o_whole, o_member, o_cell, o_nib,
                    s.inner.p, s.b, m[0]);
        a[2] = 20;
        s.inner.a = 8'h22;
        s.inner.p.lo = 4'h9;
        m[1] = 8'h41;
        c = 1;
        #1 $display("t3 %0d %h %h %h | %h %h %h", o_whole, o_member, o_cell, o_nib,
                    s.inner.p, s.b, m[0]);
        $finish(0);
    end
endmodule
