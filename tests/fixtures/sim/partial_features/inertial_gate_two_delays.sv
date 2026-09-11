module tb;
    logic source;
    wire result;
    buf #(2, 3) driver(result, source);
    initial begin
        source = 0;
        #5 source = 1;
        #5 $finish;
    end
endmodule
