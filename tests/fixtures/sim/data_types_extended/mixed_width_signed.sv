// IEEE 1800-2009 11.6-11.8: expression width and signedness depend on the
// operands; packed part-selects are unsigned even when their source is signed.
module tb #(parameter WIDTH = 2048);
    logic signed [7:0] signed_byte;
    logic [15:0] unsigned_word;
    logic signed [WIDTH-1:0] signed_wide;
    logic [WIDTH-1:0] unsigned_wide;
    logic [WIDTH-1:0] expected;
    logic mixed_unsigned_compare;
    logic all_signed_compare;
    integer failed;

    initial begin
        signed_byte = -2;
        unsigned_word = 1;
        failed = 0;
        #1;

        signed_wide = signed_byte;
        expected = '1;
        expected[7:0] = 8'hfe;
        if (signed_wide !== expected) begin
            $display("FAIL assignment-sign-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        unsigned_wide = signed_byte + unsigned_word;
        expected = '0;
        expected[7:0] = 8'hff;
        if (!failed && unsigned_wide !== expected) begin
            $display("FAIL mixed-add-unsigned WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_wide = signed_byte + $signed(8'd1);
        if (!failed && signed_wide !== {WIDTH{1'b1}}) begin
            $display("FAIL all-signed-add WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_wide = signed_byte * 16'sd2;
        expected = '1;
        expected[2:0] = 3'b100;
        if (!failed && signed_wide !== expected) begin
            $display("FAIL signed-multiply WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_wide = '1;
        signed_wide[7:0] = 8'h80;
        unsigned_wide = signed_wide[7:0];
        expected = '0;
        expected[7:0] = 8'h80;
        if (!failed && unsigned_wide !== expected) begin
            $display("FAIL part-select-unsigned WIDTH=%0d", WIDTH);
            failed = 1;
        end

        mixed_unsigned_compare = signed_byte < unsigned_word;
        all_signed_compare = signed_byte < $signed(unsigned_word);
        if (!failed &&
            (mixed_unsigned_compare !== 1'b0 || all_signed_compare !== 1'b1)) begin
            $display("FAIL comparison-signedness WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS mixed_width_signed WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
