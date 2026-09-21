// llg-test-fixture: SYN-019 SystemVerilog-2009 types, type parameters, patterns
// IEEE 1800-2009 §§6.18, 6.20.3, 7.2.1, 10.9, and 9.2.2.2: typedef, packed
// structs, type parameters, assignment patterns, and always_comb.
typedef logic [3:0] nibble_t;
typedef struct packed {
    nibble_t value;
    logic valid;
} packet_t;

module tb #(parameter type T = packet_t);
    T packet;
    localparam int WIDTH = $clog2(9);
    logic trigger;
    logic [WIDTH-1:0] result;

    always_comb begin
        packet = '{value: trigger ? 4'ha : 4'h5, valid: 1'b1};
        result = WIDTH;
    end

    initial begin
        trigger = 1'b0;
        #0;
        $display("sv_types=%h width=%0d", packet, result);
        $finish;
    end
endmodule
