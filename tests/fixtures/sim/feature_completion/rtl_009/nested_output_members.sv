// SV 23.3.3.2: fixed outputs into nested record members and array rows.
typedef struct { logic [3:0] lo; logic [3:0] hi[2]; } inner_t;
typedef struct { inner_t in[2]; int k; } outer_t;
typedef struct { logic [3:0] a; logic [3:0] b[2]; } rec_t;

module child(input int seed, output logic [3:0] o, output logic [3:0] row[2]);
    assign o = seed[3:0];
    assign row[0] = seed[7:4];
    assign row[1] = seed[11:8];
endmodule

module rec_child(input logic [3:0] k, output rec_t r, output logic [3:0] row[2]);
    always_comb begin
        r.a = k;
        r.b[0] = k + 4'd1;
        r.b[1] = k + 4'd2;
        row[0] = ~k;
        row[1] = k ^ 4'h5;
    end
endmodule

module tb;
    outer_t s;
    rec_t recs[2];
    logic [3:0] m[3][2];
    int seed = 32'h321;
    child c0(.seed(seed), .o(s.in[1].lo), .row(s.in[0].hi));
    child c1(.seed(32'h654), .o(s.in[0].lo), .row(s.in[1].hi));
    rec_child r0(.k(4'h1), .r(recs[1]), .row(m[2]));
    rec_child r1(.k(4'h3), .r(recs[0]), .row(m[0]));
    initial begin
        #1 $display("%h %h %h | %h %h %h", s.in[0].lo, s.in[0].hi[0], s.in[0].hi[1],
                    s.in[1].lo, s.in[1].hi[0], s.in[1].hi[1]);
        $display("%h %h %h | %h %h %h", recs[1].a, recs[1].b[0], recs[1].b[1],
                 recs[0].a, recs[0].b[0], recs[0].b[1]);
        $display("%h %h %h %h %b", m[2][0], m[2][1], m[0][0], m[0][1], m[1][0]);
        seed = 32'hcba;
        #1 $display("%h %h %h", s.in[1].lo, s.in[0].hi[0], s.in[0].hi[1]);
        $finish(0);
    end
endmodule
