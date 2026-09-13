module tb;
    logic source;
    wire result;
    assign #(2, 3) result = source;
    initial begin
        source = 0;
        #5 source = 1;
        #5 $finish(0);
    end
endmodule
