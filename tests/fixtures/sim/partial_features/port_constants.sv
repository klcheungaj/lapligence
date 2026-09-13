module child(input var logic [7:0] a=8'h93, output wire [7:0] b);
    assign b=a;
endmodule
module tb;
    wire [7:0] a,b,c,d,e;
    child explicit_value(8'h5a,a);
    child omitted(.b(b));
    child explicitly_open(.a(),.b(c));
    child fill_one('1,d);
    child unknown_value(8'b10xz01zx,e);
    initial begin
        #1 $display("%h %h %h %h %b",a,b,c,d,e);
        $finish(0);
    end
endmodule
