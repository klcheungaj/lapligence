module tb;
    int value;
    function automatic int adjusted(input int a, input int b = a + 1);
        return b;
    endfunction
    initial begin
        value = 41;
        #1;
        $display("%0d", adjusted(.a(value)));
        $finish(0);
    end
endmodule
