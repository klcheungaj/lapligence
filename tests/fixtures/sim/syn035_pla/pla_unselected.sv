// llg-test-fixture: tests/fixtures/sim/syn035_pla/pla_unselected.sv
// IEEE 1364-2001 §17.5 / IEEE 1800-2009 §20.17 define PLA task syntax.
// SYN-035 excludes PLA execution; this call must fail explicitly.
module tb;
    reg [0:1] personality [0:1];
    reg [0:1] input_bits;
    reg [0:0] output_bit;
    initial begin
        personality[0] = 2'b01;
        personality[1] = 2'b10;
        input_bits = 2'b01;
        output_bit = 1'b0;
        $async$and$array(personality, input_bits, output_bit);
    end
endmodule
