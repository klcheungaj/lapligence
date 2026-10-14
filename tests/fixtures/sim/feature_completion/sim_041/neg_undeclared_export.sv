// SIM-041 negative: an export declaration without the subroutine it names
// (SV 35.7: the export names a function defined in the same scope).
module tb;
    export "DPI-C" function missing_fn;

    initial $finish;
endmodule
