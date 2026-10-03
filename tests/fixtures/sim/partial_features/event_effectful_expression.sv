module tb;
    logic clk;
    int calls = 0;
    function automatic bit change(); clk=~clk; calls++; return 1; endfunction
    initial begin
        fork
            @(change()) $display("fired");
            #1 $display("result unchanged evaluated=%0d clk=%b", calls > 0, clk);
        join_any
        $finish(0);
    end
endmodule
