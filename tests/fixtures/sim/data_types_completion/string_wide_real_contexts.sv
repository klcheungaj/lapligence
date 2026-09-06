// IEEE 1800-2009 6.12.2, 6.16.10, and 6.16.15: atoreal's real result is
// converted in the assignment context of a wide integral destination. A wide
// packed actual passed to realtoa is first converted to its declared real
// argument type. The selected powers of two are exactly representable as real.
module tb;
    string power64_text = "18446744073709551616.0";
    string power100_text = "1267650600228229401496703205376.0";
    string negative_half_text = "-2.5";
    string positive_half_text = "3.5";
    logic [127:0] parsed128;
    logic [511:0] parsed512;
    logic signed [127:0] rounded_negative128;
    logic [511:0] rounded_positive512;
    logic [127:0] realtoa_source128;
    logic [511:0] realtoa_source512;
    string formatted128;
    string formatted512;

    initial begin
        parsed128 = power64_text.atoreal();
        parsed512 = power100_text.atoreal();
        rounded_negative128 = negative_half_text.atoreal();
        rounded_positive512 = positive_half_text.atoreal();
        if (parsed128 !== 128'h0000000000000001_0000000000000000
                || parsed512 !== (512'b1 << 100)
                || rounded_negative128 !== -128'sd3
                || rounded_positive512 !== 512'd4) begin
            $display("FAIL string_atoreal_wide_assignment");
            $finish;
        end

        realtoa_source128 = 128'h0000000000000001_0000000000000000;
        realtoa_source512 = 512'b1 << 100;
        formatted128.realtoa(realtoa_source128);
        formatted512.realtoa(realtoa_source512);
        if (formatted128.len() == 0
                || formatted512.len() == 0
                || formatted128.atoreal() != 18446744073709551616.0
                || formatted512.atoreal()
                    != 1267650600228229401496703205376.0) begin
            $display("FAIL string_realtoa_wide_argument");
            $finish;
        end

        $display("PASS string_wide_real_contexts");
        $finish;
    end
endmodule
