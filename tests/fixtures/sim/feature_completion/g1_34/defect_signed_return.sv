// llg-test-fixture: G1-34 rtl_no_silent_omissions (defect witness, ignored).
// IEEE 1800-2009 13.4.1/11.8.2: an assignment to the function-name return
// variable inherits the declared return type as its assignment context. For a
// signed `int` return, an 8-bit unsigned operand whose value exceeds 127 must
// be zero-extended into the 32-bit result (160), and a 9-bit sum (256) must be
// preserved. The current lowerer evaluates the RHS at the operand width and
// sign-extends (f1=-96, f2=0) while an unsigned `logic [7:0]` return is correct
// (expressions/dispatch.rs / statements/assignments.rs ownership).
module tb;
    logic [7:0] x;

    function automatic int f1(input logic [7:0] a);
        f1 = a;
    endfunction

    function automatic int f2(input logic [7:0] a, b);
        f2 = a + b;
    endfunction

    function automatic logic [7:0] g1(input logic [7:0] a);
        g1 = a;
    endfunction

    initial begin
        x = 8'hA0;
        $display("f1=%0d f2=%0d g1=%0d", f1(x), f2(8'h80, 8'h80), g1(x));
        $finish(0);
    end
endmodule
