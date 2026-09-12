// IEEE 1800-2009 21.6: an unmatched value plusarg query returns zero and
// leaves its destination unchanged; matched illegal values write the
// destination's invalid representation.
module tb;
    logic [7:0] no_match = 8'ha5;
    logic [7:0] malformed = 8'h5a;
    logic [7:0] empty_numeric = 8'hff;
    real real_value = 2.5;
    string string_value = "keep";
    logic no_match_result;
    logic malformed_result;
    logic empty_numeric_result;
    logic real_result;
    logic string_result;

    initial begin
        no_match_result = $value$plusargs("missing=%d", no_match);
        malformed_result = $value$plusargs("bad=%d", malformed);
        empty_numeric_result = $value$plusargs("zero=%0d", empty_numeric);
        real_result = $value$plusargs("real=%f", real_value);
        string_result = $value$plusargs("string=%s", string_value);
        $display("no_match=%b result=%0d malformed=%h result=%0d zero=%0d result=%0d",
                 no_match, no_match_result, malformed, malformed_result,
                 empty_numeric, empty_numeric_result);
        $display("real=%.1f result=%0d string=<%s> result=%0d", real_value,
                 real_result, string_value, string_result);
        $finish(0);
    end
endmodule
