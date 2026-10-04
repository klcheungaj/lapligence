// IEEE 1364-2001 8.6 (Syntax 8-2); IEEE 1800-2009 A.5.4: every UDP input
// terminal is an expression, so an empty connection is not a terminal.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        1 ? : 1;
    endtable
endprimitive

module tb;
    reg a;
    wire y;
    p u(y, a, );
    initial begin
        a = 0;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
