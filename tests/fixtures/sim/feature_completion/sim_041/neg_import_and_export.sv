// SIM-041 negative: one C identifier both imported and exported (SV 35.4:
// all DPI subroutines with one c_identifier name the same C function).
module tb;
    import "DPI-C" context function int twice(input int a);
    export "DPI-C" twice = function sv_twice;

    function int sv_twice(input int a);
        return 2 * a;
    endfunction

    initial $finish;
endmodule
