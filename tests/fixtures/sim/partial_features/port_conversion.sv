module child(input logic signed [15:0] s, input bit [7:0] b,
             input logic [8:0] sum, output wire [15:0] so,
             output wire [7:0] bo, output wire [8:0] sumo);
    assign so=s; assign bo=b; assign sumo=sum;
endmodule
module tb;
    logic signed [7:0] negative;
    logic [7:0] four,a,b;
    wire [15:0] so;
    wire [7:0] bo;
    wire [8:0] sumo;
    child c(negative,four,a+b,so,bo,sumo);
    initial begin
        negative=-2; four=8'b1x0z10xz; a=255; b=1;
        #1 $display("%h %b %h",so,bo,sumo);
        negative=-128; four='z; a=128; b=128;
        #1 $display("%h %b %h",so,bo,sumo);
        $finish;
    end
endmodule
