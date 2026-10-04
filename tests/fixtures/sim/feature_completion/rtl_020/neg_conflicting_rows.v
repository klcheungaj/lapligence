// IEEE 1364-2001 8.1.6; IEEE 1800-2009 29.3.5: overlapping rows that give
// the same input combination different outputs are illegal. Here 0 0
// matches both rows.
primitive p(y, a, b);
    output y;
    input a, b;
    table
        0 ? : 0;
        ? 0 : 1;
    endtable
endprimitive

module tb;
    reg a, b;
    wire y;
    p u(y, a, b);
    initial begin
        a = 0; b = 1;
        #1 $display("%b", y);
        $finish(0);
    end
endmodule
