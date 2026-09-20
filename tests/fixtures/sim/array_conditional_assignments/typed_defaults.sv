// Typed row defaults preserve whole-byte values, unlike untyped bit defaults.
module tb;
    typedef logic [7:0] descending_row_t [-1:0];
    typedef logic [7:0] ascending_row_t [1:0];
    descending_row_t descending [3:1];
    ascending_row_t ascending [-2:0];
    logic [7:0] default_byte;
    logic [7:0] packed_byte;
    logic signed [7:0] packed_signed;
    logic [0:1][3:0] packed_rows;
    initial begin
        default_byte = 8'hdd;
        descending = '{3: '{8'h31, 8'h32},
                       default: descending_row_t'{default: default_byte}};
        default_byte = 8'hee;
        ascending = '{0: '{8'h01, 8'h02},
                      default: ascending_row_t'{default: default_byte}};
        default_byte = 0;
        if (descending[3][-1] !== 8'h31 || descending[3][0] !== 8'h32)
            $fatal(1, "descending explicit row moved");
        if (descending[2][-1] !== 8'hdd || descending[2][0] !== 8'hdd ||
            descending[1][-1] !== 8'hdd || descending[1][0] !== 8'hdd)
            $fatal(1, "typed descending row lost its byte defaults");
        if (ascending[0][1] !== 8'h01 || ascending[0][0] !== 8'h02)
            $fatal(1, "ascending explicit row moved");
        if (ascending[-2][1] !== 8'hee || ascending[-2][0] !== 8'hee ||
            ascending[-1][1] !== 8'hee || ascending[-1][0] !== 8'hee)
            $fatal(1, "typed ascending row lost its byte defaults");
        packed_byte = '{default: 1'b1};
        packed_signed = '{default: 1'b1};
        packed_rows = '{'{default: 1'b1}, 4'ha};
        if (packed_byte !== 8'hff || packed_signed != -1 || packed_rows !== 8'hfa)
            $fatal(1, "packed pattern lost element sizing, order or result signedness");
        $display("typed_defaults passed");
        $finish(0);
    end
endmodule
