module child(input logic [3:0] a, output wire [3:0] b);
    assign b=a;
endmodule
module tb;
    logic [3:0] x,y;
    wire [11:0] bus;
    child low(x,bus[3:0]);
    child high(y,bus[11:8]);
    initial begin
        x=4'h5; y=4'ha;
        #1 $display("%h",bus);
        x=4'hc; y=4'h3;
        #1 $display("%h",bus);
        $finish;
    end
endmodule
