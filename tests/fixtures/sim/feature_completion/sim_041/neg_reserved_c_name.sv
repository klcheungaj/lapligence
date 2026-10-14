// SIM-041 negative: an export whose C identifier the generated model or its
// runtime already defines (svdpi.h routine name).
module tb;
    export "DPI-C" svGetScope = function get_scope;

    function int get_scope();
        return 0;
    endfunction

    initial $finish;
endmodule
