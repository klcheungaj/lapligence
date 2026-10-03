module tb;
    logic [128:0] a;
    logic [64:0] part;
    initial begin
        a = 19;
        #1;
        part = a[64:0];
        $display("part=%0d", part);
        $finish(0);
    end
endmodule
