// IEEE 1800-2009 6.21/13.4.2: a static local initializes once, while an
// automatic local is recreated on every activation.
module tb;
    int side = 0;

    function automatic int bump();
        side = side + 1;
        bump = side;
    endfunction

    function automatic int f(input int x);
        static int s = bump();
        automatic int a = 0;
        s = s + 1;
        a = a + x;
        f = s * 100 + a;
    endfunction

    initial begin
        $display("%0d %0d %0d", f(1), f(1), f(2));
        $display("side=%0d", side);
        $finish(0);
    end
endmodule
