module tb;
    logic clk;
    int calls = 0;
    function automatic bit change(); clk=~clk; calls++; return 1; endfunction
    initial begin
        clk = 0;
        fork
            @(posedge clk iff change()) $display("qualified calls=%0d clk=%b", calls, clk);
            #1 clk = 1;
        join
        $finish(0);
    end
endmodule
