// IEEE 1800-2009 21.6: command-line plusargs use literal prefix matching and
// typed conversion into the supplied variable.
module tb;
    localparam logic [8*8-1:0] HEX_FORMAT = "hex=%0X";
    logic test_hit;
    logic [31:0] test_result;
    logic [8*9-1:0] dynamic_format;
    logic signed [15:0] decimal_value;
    logic [127:0] hex_value;
    logic [5:0] binary_value;
    logic [8:0] octal_value;
    bit [7:0] two_state_value;
    real fixed_real;
    real exponent_real;
    real general_real;
    shortreal short_real;
    string text_value;
    string empty_value;
    logic [63:0] packed_text;
    logic [7:0] repeat_value;
    logic [7:0] collision_value;
    logic [7:0] percent_value;
    logic [7:0] direct_value;
    logic [7:0] condition_value;
    logic condition_seen;
    string dynamic_pattern;

    initial begin
        dynamic_pattern = "mode";
        dynamic_format = "repeat=%d";
        test_hit = $test$plusargs(dynamic_pattern);
        test_result = $value$plusargs(dynamic_format, repeat_value);
        test_result = $value$plusargs("dec=%d", decimal_value);
        test_result = $value$plusargs(HEX_FORMAT, hex_value);
        test_result = $value$plusargs("bits=%b", binary_value);
        test_result = $value$plusargs("oct=%o", octal_value);
        test_result = $value$plusargs("state=%b", two_state_value);
        test_result = $value$plusargs("fixed=%f", fixed_real);
        test_result = $value$plusargs("exponent=%e", exponent_real);
        test_result = $value$plusargs("general=%g", general_real);
        test_result = $value$plusargs("short=%f", short_real);
        test_result = $value$plusargs("text=%s", text_value);
        test_result = $value$plusargs("empty=%s", empty_value);
        test_result = $value$plusargs("packed=%s", packed_text);
        test_result = $value$plusargs("collision=%d", collision_value);
        test_result = $value$plusargs("literal%%=%d", percent_value);
        $value$plusargs("direct=%d", direct_value);
        if ($value$plusargs("condition=%d", condition_value))
            condition_seen = 1'b1;
        $display("hit=%0d repeat=%0d dec=%0d", test_hit, repeat_value, decimal_value);
        $display("hex=%h bits=%b oct=%o state=%b", hex_value, binary_value,
                 octal_value, two_state_value);
        $display("real=%.2f %.2e %.3f short=%.2f", fixed_real, exponent_real,
                 general_real, short_real);
        $display("text=<%s> empty=<%s> packed=%h collision=%0d percent=%0d", text_value,
                 empty_value, packed_text, collision_value, percent_value);
        $display("direct=%0d condition=%0d seen=%0d", direct_value, condition_value,
                 condition_seen);
        $finish(0);
    end
endmodule
