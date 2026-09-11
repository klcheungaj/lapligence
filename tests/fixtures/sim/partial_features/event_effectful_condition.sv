module tb;
    logic clk;
    function automatic bit change(); clk=~clk; return 1; endfunction
    initial @(posedge clk iff change()) $finish;
endmodule
