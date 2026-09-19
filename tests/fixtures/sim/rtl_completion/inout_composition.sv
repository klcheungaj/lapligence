module leaf(inout wire [7:0] port, input logic [7:0] drive);
    wire [7:0] view;
    alias port = view;
    assign view = drive;
endmodule
module forward(inout wire [7:0] port, input logic [7:0] drive);
    leaf nested(port, drive);
endmodule
module tb;
    wire [7:0] lanes[2];
    wire [15:0] bus;
    logic [7:0] a, b;
    forward first(lanes[0], a);
    leaf second(lanes[0], b);
    leaf selected(bus[11:4], a);
    assign lanes[0] = 8'hzz;
    assign bus[3:0] = 4'h5;
    initial begin
        a=8'h5a; b='z;
        #1;
        $display("lane=%h sibling=%h bus=%h", lanes[0], lanes[1], bus);
        a='z; b=8'ha5;
        #1;
        $display("lane=%h sibling=%h bus=%h", lanes[0], lanes[1], bus);
        $finish(0);
    end
endmodule
