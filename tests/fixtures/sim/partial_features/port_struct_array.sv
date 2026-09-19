// IEEE 1800-2009 6.22 and 23.3.3: a fixed unpacked structure containing a
// fixed unpacked array crosses a module port by value; the child observes a
// deep copy and always_comb is sensitive to each leaf.
typedef struct {
    logic [7:0] lane;
    logic [7:0] bytes [0:2];
} packet_t;

module child(input packet_t in_packet, output packet_t out_packet);
    always_comb begin
        out_packet.lane = in_packet.lane + 8'd1;
        out_packet.bytes[0] = in_packet.bytes[0] + 8'd1;
        out_packet.bytes[1] = in_packet.bytes[1] + 8'd1;
        out_packet.bytes[2] = in_packet.bytes[2] + 8'd1;
    end
endmodule

module tb;
    packet_t source;
    packet_t result;

    child dut(.in_packet(source), .out_packet(result));

    initial begin
        source.lane = 8'h10;
        source.bytes[0] = 8'h20;
        source.bytes[1] = 8'h30;
        source.bytes[2] = 8'h40;
        #1;
        if (result.lane !== 8'h11 || result.bytes[0] !== 8'h21
                || result.bytes[1] !== 8'h31 || result.bytes[2] !== 8'h41) begin
            $display("FAIL port_initial");
                $finish(0);
        end
        // Only one leaf changes; the other three outputs keep their values.
        source.bytes[1] = 8'h99;
        #1;
        if (result.lane !== 8'h11 || result.bytes[0] !== 8'h21
                || result.bytes[1] !== 8'h9a || result.bytes[2] !== 8'h41) begin
            $display("FAIL port_leaf_update %h %h %h %h", result.lane,
                     result.bytes[0], result.bytes[1], result.bytes[2]);
                $finish(0);
        end
        $display("PASS port_struct_array");
            $finish(0);
    end
endmodule
