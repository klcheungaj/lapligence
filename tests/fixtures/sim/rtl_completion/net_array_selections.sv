module leaf(inout wire [3:0] port, input logic [3:0] drive);
    assign port = drive;
endmodule
module tb;
    wire [7:0] lanes[1:-1];
    logic [3:0] a, b;
    leaf hi(lanes[0][7:4], a);
    leaf lo(lanes[0][3:0], b);
    assign lanes[0][6:5] = 2'bzz;
    assign lanes[0][1 +: 2] = 2'bzz;
    initial begin
        a=4'ha; b=4'h5;
        #1;
        $display("lane=%h siblings=%h,%h", lanes[0], lanes[1], lanes[-1]);
        a='z; b=4'h3;
        #1;
        $display("lane=%h siblings=%h,%h", lanes[0], lanes[1], lanes[-1]);
        $finish(0);
    end
endmodule
