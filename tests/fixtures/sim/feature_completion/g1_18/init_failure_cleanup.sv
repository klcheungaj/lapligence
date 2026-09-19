// IEEE 1800-2009 6.8: declaration initializer calls own fixed local arrays.
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
