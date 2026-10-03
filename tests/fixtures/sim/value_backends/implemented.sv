module tb;
    logic [128:0] a, b, product;
    initial begin
        a = 17;
        b = 19;
        product = a * b;
        #1;
        $display("product=%0d", product);
        $finish(0);
    end
endmodule
