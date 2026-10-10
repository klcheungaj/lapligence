// SIM-040 negative: open arrays are allowed only for imports (35.5.6.1).
module tb;
    export "DPI-C" function neg_export;
    function void neg_export(input int a []);
    endfunction
endmodule
