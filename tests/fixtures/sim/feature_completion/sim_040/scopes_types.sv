// SIM-040: one C function imported from several scopes, including an
// open-array import whose actuals differ per scope (SV 35.5.4, 35.5.6.1),
// and packed formals of enum, packed-structure and integer type (H.7.3,
// H.7.4). Build scopes_types.c into a shared library and pass it with
// --dpi-lib.
module sub;
    import "DPI-C" function int st_sum(input int a []);
    import "DPI-C" function int st_twice(input int b);
    int z [0:2];
    initial begin
        z = '{5, 6, 7};
        #1 $display("sub sum=%0d twice=%0d", st_sum(z), st_twice(4));
    end
endmodule

module tb;
    typedef enum {RED, GREEN, BLUE} color_t;
    typedef enum logic [1:0] {L0, L1, LX = 2'bx1} lcolor_t;
    typedef struct packed { bit [3:0] hi; logic [3:0] lo; } ps_t;
    import "DPI-C" function int st_sum(input int a []);
    import "DPI-C" pure function int st_pure(input int a []);
    import "DPI-C" function int st_twice(input int a);
    import "DPI-C" function int st_enum(input color_t c, input lcolor_t l, output color_t o);
    import "DPI-C" function void st_packed(inout ps_t p, input integer i);
    import "DPI-C" function void st_views(input bit [7:0] a, input logic [7:0] b);

    int m [1:0][3:5];
    int big [1:3];
    color_t c;
    ps_t p;
    logic [7:0] lx;

    sub s1();

    initial begin
        foreach (m[i, j]) m[i][j] = i * 10 + j;
        $display("slice=%0d twice=%0d", st_sum(m[1]), st_twice(3));
        big = '{100, 200, 300};
        $display("pure=%0d %0d", st_pure(big), st_pure(big));
        $display("enum=%0d", st_enum(GREEN, LX, c));
        $display("enum out=%s", c.name());
        p = 8'hax;
        st_packed(p, 32'shffff_fffe);
        $display("packed=%h", p);
        lx = 8'b1x0z_1100;
        st_views(lx, lx);
        #2 $finish;
    end
endmodule
