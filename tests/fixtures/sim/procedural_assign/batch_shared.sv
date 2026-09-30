module worker;
    wire [7:0] source = 8'h5a;
    logic [7:0] a, b, c, d;
    initial begin
        #1;
        assign a = source;
        assign b = source;
        assign c = source;
        assign d = source;
        #1;
        $display("CHECK: shared=%h %h %h %h", a, b, c, d);
        deassign d;
        d = 8'ha5;
        #1;
        $display("CHECK: freed=%h", d);
    end
endmodule

module tb;
    worker u0();
    worker u1();
    worker u2();
    worker u3();
    initial begin
        #4;
        $finish(0);
    end
endmodule
