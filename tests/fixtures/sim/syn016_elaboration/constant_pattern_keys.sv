// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/constant_pattern_keys.sv
// Requalify the N03 key metadata and N11 selector-Z gates together:
// the key and width are constant, but neither payload assignment is.
package key_pkg;
    function automatic int choose(input logic [2:0] selector);
        casez (selector)
            3'b101: return 1;
            default: return 2;
        endcase
    endfunction
    localparam int KEY = choose(3'bz01);
endpackage
module tb;
    import key_pkg::*;
    typedef struct {
        logic [7:0] data [2:0];
        logic [3:0] tag;
    } packet_t;
    packet_t packet;
    logic [7:0] source_value;
    logic [2:0] selector;
    logic [KEY+2:0] sized_by_key;
    initial begin
        selector = 3'bz01;
        source_value = 8'h3c;
        packet = '{data: '{key_pkg::KEY: source_value, default: 8'ha0}, tag: 4'd3};
        #1;
        $display("keys=%0d width=%0d data=%h/%h/%h tag=%h runtime=%0d",
                 KEY, $bits(sized_by_key), packet.data[2], packet.data[1],
                 packet.data[0], packet.tag, choose(selector));
        source_value = 8'hc3;
        packet = '{data: '{KEY: source_value, default: 8'h0a}, tag: 4'd5};
        #1;
        $display("keys=%0d width=%0d data=%h/%h/%h tag=%h runtime=%0d",
                 KEY, $bits(sized_by_key), packet.data[2], packet.data[1],
                 packet.data[0], packet.tag, choose(selector));
        $finish(0);
    end
endmodule
