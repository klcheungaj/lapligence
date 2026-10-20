// Decision S40-D6: an svdpi.h open-array element access whose number of
// indices differs from the array's number of unpacked dimensions is a
// run-time error that ends the simulation: the import does not complete in
// SystemVerilog, nothing after the call runs, and the final procedures run
// (llg: an error on stderr, exit status 1). The access itself does nothing;
// llg cannot unwind the foreign frame, so C code after the access still
// runs in llg, and this case performs the access last.
//
// IEEE 1800-2009 H.12.4 (SystemVerilog-1800-2009.txt L73337-73340):
//   "Nonetheless, the addresses of individual elements of an array shall be
//   always supported."
// Annex H gives one accessor per number of indices and does not say what a
// different count does; llg follows the industry-standard behaviour.
//
// Build S40-D6_wrong_index_count_ends_run.c into a shared library and load
// it with the simulator's DPI library option.
module tb;
    import "DPI-C" function void d6_access(inout bit b [][]);
    bit b [0:1][3:2];

    final begin
        $display("final block");
    end

    initial begin
        $display("before call");
        d6_access(b);
        $display("after call");
        $finish;
    end
endmodule
