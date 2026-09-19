// IEEE 1800-2009 6.8: a declaration initializer that cannot be lowered must
// fail code generation without leaving a partially registered model that a
// driver could run. The callee's automatic unpacked-array local is outside
// the supported subset.
module tb;
    function automatic int bad();
        int acc [0:1];
        acc[0] = 1;
        bad = acc[0];
    endfunction

    int x = bad();

    initial begin
        $display("x=%0d", x);
        $finish(0);
    end
endmodule
