typedef struct { logic [7:0] high; logic [7:0] low; } pair_t;
module pair_driver(inout wire pair_t port, input logic [7:0] drive);
    assign port.high = drive;
    assign port.low = 8'hzz;
endmodule
module byte_driver(inout wire [7:0] port, input logic [7:0] drive);
    assign port = drive;
endmodule
module tb;
    wire pair_t bus;
    logic [7:0] a, b;
    pair_driver first(bus, a);
    byte_driver second(bus.low, b);
    initial begin
        a=8'ha5; b=8'h5a;
        #1;
        $display("bus=%h,%h", bus.high, bus.low);
        a=8'h12; b=8'h34;
        #1;
        $display("bus=%h,%h", bus.high, bus.low);
        $finish(0);
    end
endmodule
