module tb;
    logic [3:0] a, b;
    wire [3:0] direct, generated;
    and outer[3:0](direct, a, b);
    if (1) begin : scope
        or inner[3:0](generated, a, b);
    end
    initial begin
        a = 4'b1010;
        b = 4'b1100;
        #1 $display("direct=%b generated=%b", direct, generated);
        a = 4'b0101;
        #1 $display("direct=%b generated=%b", direct, generated);
        $finish(0);
    end
endmodule
