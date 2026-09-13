module tb;
    logic clk;
    function automatic bit change(); clk=~clk; return 1; endfunction
    initial @(change()) $finish(0);
endmodule
