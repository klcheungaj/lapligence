// A record whose member array is a column crosses the DPI as one structure,
// with the C layout of SV H.7.2 and H.11.4: the columns are an internal form,
// so foreign code sees the same bytes as for any other record, for import and
// export formals of every direction (SV 35.6.1). Build record_columns.c into
// a shared library and pass it with --dpi-lib.
module tb;
    typedef struct { int id; logic [3:0] tag; byte data [16]; bit [39:0] wide; } col_t;
    typedef struct { shortint n; logic [3:0] m [3:0][7:0]; } grid_t;

    import "DPI-C" function void rc_in(input col_t c, input grid_t g);
    import "DPI-C" function void rc_out(output col_t c);
    import "DPI-C" function void rc_inout(inout col_t c, inout grid_t g);
    import "DPI-C" context task rc_drive();

    export "DPI-C" function ex_auto;
    export "DPI-C" function ex_static;
    export "DPI-C" task ex_wait;

    col_t c;
    grid_t g;
    col_t d;
    int i;

    function automatic void show_col(string name, col_t v);
        $display("%s id=%0d tag=%b wide=%h data=%0d,%0d,%0d,%0d", name, v.id, v.tag, v.wide,
                 v.data[0], v.data[1], v.data[7], v.data[15]);
    endfunction

    function automatic void show_grid(string name, grid_t v);
        $display("%s n=%0d %h %h %h %h", name, v.n, v.m[3][7], v.m[3][0], v.m[0][7], v.m[0][0]);
    endfunction

    function automatic void ex_auto(input col_t in, output col_t out, inout grid_t io);
        show_col("ex_auto in", in);
        show_grid("ex_auto io", io);
        out = in;
        out.id = in.id + 1;
        out.data[1] = -in.data[1];
        io.n = io.n + 1;
        io.m[2][3] = 4'b1x0z;
    endfunction

    function void ex_static(input col_t in, output col_t out);
        out = in;
        out.tag = ~in.tag;
    endfunction

    task automatic ex_wait(inout col_t v);
        #2;
        v.data[15] = v.data[15] + 1;
    endtask

    initial begin
        for (i = 0; i < 16; i++) c.data[i] = byte'(i - 8);
        c.id = -5;
        c.tag = 4'b01xz;
        c.wide = 40'h12_3456_789a;
        g.n = 300;
        for (i = 0; i < 8; i++) g.m[3][i] = 4'(unsigned'(i));
        g.m[0][7] = 4'hc;
        g.m[0][0] = 4'bx1z0;
        rc_in(c, g);
        rc_out(d);
        show_col("out", d);
        rc_inout(c, g);
        show_col("inout", c);
        show_grid("inout", g);
        rc_drive();
        $display("%0t done", $time);
        $finish(0);
    end
endmodule
