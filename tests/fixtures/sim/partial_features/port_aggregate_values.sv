typedef struct {
    logic [7:0] bits;
    real amount;
} packet_t;

module child(input packet_t in_packet, output packet_t out_packet);
    assign out_packet.bits = in_packet.bits + 8'd1;
    assign out_packet.amount = in_packet.amount + 0.5;
endmodule

module tb;
    packet_t source;
    packet_t result;

    child dut(.in_packet(source), .out_packet(result));

    initial begin
        source.bits = 8'h10;
        source.amount = 1.25;
        #1 $display("%h %.2f", result.bits, result.amount);
        source.bits = 8'ha0;
        source.amount = 3.0;
        #1 $display("%h %.2f", result.bits, result.amount);
        $finish(0);
    end
endmodule
