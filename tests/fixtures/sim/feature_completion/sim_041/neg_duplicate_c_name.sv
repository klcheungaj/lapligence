// SIM-041 negative: two exports of one scope with the same C identifier
// (SV 35.7).
module tb;
    export "DPI-C" one = function f;
    export "DPI-C" one = function g;

    function int f(input int a);
        return a;
    endfunction

    function int g(input int a);
        return a;
    endfunction

    initial $finish;
endmodule
