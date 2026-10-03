// IEEE 1800-2009 6.5, 7.2, 10.3 and 10.9: nested positional patterns publish
// into packed nets, unpacked-structure nets and members, and disjoint
// variable cells and members.
module tb;
    typedef struct { logic [3:0] a; logic [7:0] b; } rec_t;
    typedef struct { rec_t r; logic [3:0] t; } outer_t;
    typedef struct { logic [3:0] a; logic [3:0] b; } half_t;
    typedef logic [1:0][1:0][3:0] quad_t;
    wire [3:0] a, b, c, d;
    wire rec_t sn;
    wire [3:0] t;
    wire half_t sh;
    wire [3:0] th;
    rec_t sv;
    logic [3:0] arr[0:2];
    quad_t x = 16'h1234;
    outer_t o = '{'{4'h5, 8'h67}, 4'h8};
    logic [2:0][3:0] z = 12'h9ab;
    assign '{'{a, b}, '{c, d}} = x;
    assign '{sn, t} = o;
    assign '{sh.a, th, sh.b} = z;
    assign '{sv.a, arr[1]} = x[1];
    assign sv.b = o.r.b;
    initial arr[0] = 4'h9;
    initial begin
        #1 $display("%h %h %h %h | %h %h %h | %h %h %h", a, b, c, d, sn.a, sn.b, t,
                    sh.a, th, sh.b);
        $display("%h %h %h %h %h", sv.a, sv.b, arr[0], arr[1], arr[2]);
        x = 16'hfedc;
        o.r.b = 8'h9z;
        o.t = 4'h0;
        z = 12'h4z6;
        #1 $display("%h %h %h %h | %h %h %h | %h %h %h", a, b, c, d, sn.a, sn.b, t,
                    sh.a, th, sh.b);
        $display("%h %h %h %h %h", sv.a, sv.b, arr[0], arr[1], arr[2]);
        $finish(0);
    end
endmodule
