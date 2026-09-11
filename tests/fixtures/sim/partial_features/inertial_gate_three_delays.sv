module tb;
    logic source, enable;
    wire result;
    bufif1 #(2, 3, 4) driver(result, source, enable);
    initial begin
        source = 0;
        enable = 1;
        #5 source = 1;
        #5 $finish;
    end
endmodule
