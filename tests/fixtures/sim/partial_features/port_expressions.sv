module child(input logic [7:0] a, output wire [7:0] b);
    assign b=a;
endmodule
module tb;
    logic [7:0] x,y;
    logic [2:0] i;
    wire [7:0] a,b,c;
    child sum(x+y,a);
    child parts({x[3:0],y[7:4]},b);
    child selected({7'b0,x[i]},c);
    initial begin
        x=8'h82; y=8'h35; i=1;
        #1 $display("%h %h %h",a,b,c);
        y=8'h41;
        #1 $display("%h %h %h",a,b,c);
        i=0; x=8'h14;
        #1 $display("%h %h %h",a,b,c);
        $finish;
    end
endmodule
