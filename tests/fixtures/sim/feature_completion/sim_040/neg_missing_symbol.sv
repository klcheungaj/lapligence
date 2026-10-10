// SIM-040 negative: an import that no --dpi-lib library defines is an explicit link
// error that names the import. Build neg_missing_symbol.c (which defines
// only neg_present) into a shared library and pass it with --dpi-lib.
module tb;
    import "DPI-C" function int neg_present(input int a);
    import "DPI-C" function int neg_absent(input int a);
    initial $display("%0d %0d", neg_present(1), neg_absent(2));
endmodule
