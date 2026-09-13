module child #(parameter logic [7:0] P=8'h11)
    (input logic [7:0] a=P, output wire [7:0] y);
    assign y=a;
endmodule
module tb;
    wire [7:0] a,b;
    child #(.P(8'h55)) first(.y(a));
    child #(.P(8'haa)) second(.y(b));
    for (genvar i=0;i<2;i=i+1) begin:g
        wire [7:0] y;
        child c(i+4,y);
    end
    initial begin
        #1 $display("%h %h %h %h",a,b,g[0].y,g[1].y);
        $finish(0);
    end
endmodule
