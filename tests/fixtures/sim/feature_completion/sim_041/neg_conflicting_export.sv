// SIM-041 negative: one C identifier exported from two scopes with different
// signatures (SV 35.4).
module a_m;
    export "DPI-C" same = function f;

    function int f(input int a);
        return a;
    endfunction
endmodule

module b_m;
    export "DPI-C" same = function f;

    function int f(input shortint a);
        return a;
    endfunction
endmodule

module tb;
    a_m ua();
    b_m ub();

    initial $finish;
endmodule
