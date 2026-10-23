// SIM-041 / decision S13-D1: the output and inout actuals of a DPI-C import
// are written when the import returns, into the targets their selectors name
// then. The C side calls an exported function that changes `i` before it
// writes the formals. Build copy_out_targets.c into a shared library and pass
// it with --dpi-lib.
module tb;
    import "DPI-C" context function void co_output(output int x, input int ni);
    import "DPI-C" context function void co_inout(inout int x, input int ni);
    import "DPI-C" context function int co_result(output bit x, input int ni);
    export "DPI-C" function set_i;

    int a[4];
    bit [7:0] v;
    int i, r;

    function void set_i(input int ni);
        i = ni;
    endfunction

    initial begin
        a = '{0, 0, 0, 0};
        i = 0;
        co_output(a[i], 3);
        $display("output %0d %0d %0d %0d", a[0], a[1], a[2], a[3]);
        a = '{1, 2, 3, 4};
        i = 0;
        co_inout(a[i], 2);
        $display("inout %0d %0d %0d %0d", a[0], a[1], a[2], a[3]);
        v = 8'h00;
        i = 1;
        r = co_result(v[i], 6);
        $display("result %0d bit %b", r, v);
        $finish;
    end
endmodule
