module leaf(ref logic [64:0] a);
    initial begin
        #2 a=65'h1fedcba9876543210;
        $display("child %h %h",a,tb.a);
    end
endmodule
module middle(ref logic [64:0] a);
    leaf l(a);
endmodule
module tb;
    logic [64:0] a;
    middle m(a);
    initial begin
        a=0;
        #1 a=65'h10000000000000001;
        $display("parent %h %h",m.a,m.l.a);
        #2 a[3:0]=4'hf;
        $display("updated %h %h",m.a,m.l.a);
        $finish;
    end
endmodule
