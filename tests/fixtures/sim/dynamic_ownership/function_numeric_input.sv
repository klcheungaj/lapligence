module tb;
    int value;
    function automatic int add1(input int a);
        return a + 1;
    endfunction
    initial begin
        value = 41;
        #1;
        $display("%0d", add1(value));
        $finish(0);
    end
endmodule
