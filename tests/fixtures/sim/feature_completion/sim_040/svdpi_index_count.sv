// SIM-040 A03: an svdpi.h element access with a number of indices other
// than the array's unpacked dimensions is a fatal error (decision S40-D6):
// the import's C code cannot be unwound, so it continues with NULL pointers,
// default reads and ignored writes; the run ends when the import returns,
// the final procedure runs and the exit status is 1. Build
// svdpi_index_count.c into a shared library and pass it with --dpi-lib.
module tb;
    import "DPI-C" function void ic_access(inout logic l [], inout bit b [][]);
    logic l [0:1];
    bit b [0:1][0:1];

    final $display("final l=%b%b b=%b%b", l[0], l[1], b[0][0], b[1][1]);

    initial begin
        l = '{1'b1, 1'b0};
        b = '{'{1'b1, 1'b0}, '{1'b0, 1'b1}};
        ic_access(l, b);
        $display("after the call");
        $finish;
    end
endmodule
