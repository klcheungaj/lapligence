// IEEE 1800-2009 6.16.10 and 6.16.15: atoreal scans a real-number prefix and
// returns zero when no digits are encountered. realtoa produces an ASCII real
// representation inverse to atoreal; its exact textual precision is not fixed.
module tb;
    string numeric_prefix = "12.5trailing";
    string whitespace_sign_exponent = " \t-1.25e2trailing";
    string positive_sign_exponent = "+6.25E-1stop";
    string invalid = "not-a-number";
    string formatted_positive;
    string formatted_negative;

    initial begin
        if (numeric_prefix.atoreal() != 12.5
                || whitespace_sign_exponent.atoreal() != -125.0
                || positive_sign_exponent.atoreal() != 0.625
                || invalid.atoreal() != 0.0) begin
            $display("FAIL string_atoreal");
            $finish;
        end

        formatted_positive.realtoa(37.125);
        formatted_negative.realtoa(-0.5);
        if (formatted_positive.len() == 0
                || formatted_negative.len() == 0
                || formatted_positive.atoreal() != 37.125
                || formatted_negative.atoreal() != -0.5) begin
            $display("FAIL string_realtoa");
            $finish;
        end

        $display("PASS string_real_conversion");
        $finish;
    end
endmodule
