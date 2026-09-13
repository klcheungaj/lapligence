module tb;
    initial begin wait (0); $display("unexpected zero"); end
    initial begin wait (1'bx); $display("unexpected unknown"); end
    initial begin wait (1); $display("ready %0t",$time); end
    initial begin #3 $display("later %0t",$time); $finish(0); end
endmodule
