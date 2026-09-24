// llg-test-fixture: IEEE 1800-2009 §§6.20, 7.3.1, 11.2.1.
module tb;
    typedef union packed {
        logic [7:0] octets;
        logic [1:0][3:0] nibbles;
    } union8_t;

    parameter union8_t BASE = 8'hA5;
    localparam logic [7:0] PROJECTED = BASE.octets;
    localparam logic [3:0] LOW_NIBBLE = BASE.nibbles[0];
    typedef logic [BASE.octets[7:4]-1:0] projected_width_t;

    union8_t runtime_view;
    logic [7:0] runtime_projection;
    logic [3:0] runtime_low_nibble;

    initial begin
        runtime_view = 8'h3C;
        runtime_projection = runtime_view.octets;
        runtime_low_nibble = runtime_view.nibbles[0];
        $display("base=%h low=%h elaborated_width=%0d", PROJECTED, LOW_NIBBLE, $bits(projected_width_t));
        $display("runtime=%h low=%h", runtime_projection, runtime_low_nibble);
        $finish(0);
    end
endmodule
