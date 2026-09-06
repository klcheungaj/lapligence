// IEEE 1800-2009 5.7.2 and 6.16: string method arguments use their declared
// integer/byte types, real-to-integer ties round away from zero, integral casts
// to string remove NUL bytes, and packed string casts right-justify and pad.
module tb;
    typedef logic [127:0] packed128_t;
    typedef logic [135:0] packed136_t;

    string chars;
    string number;
    string stripped;
    string original;
    string copied;
    string packed_source;
    string shown;
    logic [63:0] wide_index;
    logic [63:0] wide_character;
    logic [63:0] wide_integer;
    logic [47:0] embedded_nuls;
    packed128_t packed128;
    packed136_t packed136;

    initial begin
        chars = "abcd";
        wide_index = 64'h0000_0001_0000_0001;
        wide_character = 64'h1234_5678_9abc_de5a;
        chars.putc(wide_index, wide_character);
        chars.putc(2, '1);
        if (chars.len() !== 4 || chars.getc(0) !== 8'h61 ||
            chars.getc(1) !== 8'h5a || chars.getc(2) !== 8'hff ||
            chars.getc(3) !== 8'h64) begin
            $display("FAIL string_conversions declared_arguments");
            $finish;
        end

        chars.putc(-1, 8'h51);
        chars.putc(32, 8'h51);
        if (chars.getc(-1) !== 8'h00 || chars.getc(32) !== 8'h00 ||
            chars[-1] !== 8'h00 || chars[32] !== 8'h00 ||
            chars.getc(0) !== 8'h61 || chars.getc(3) !== 8'h64) begin
            $display("FAIL string_conversions out_of_bounds");
            $finish;
        end

        wide_integer = 64'h0000_0001_0000_002a;
        number.itoa(wide_integer);
        if (number != "42") begin
            $display("FAIL string_conversions wide_to_integer");
            $finish;
        end
        number.itoa('1);
        if (number != "-1") begin
            $display("FAIL string_conversions unbased_fill_integer");
            $finish;
        end
        number.itoa(2.5);
        if (number != "3") begin
            $display("FAIL string_conversions positive_real_rounding");
            $finish;
        end
        number.itoa(-1.5);
        if (number != "-2") begin
            $display("FAIL string_conversions negative_real_rounding");
            $finish;
        end

        embedded_nuls = 48'h41_00_42_00_43_00;
        stripped = string'(embedded_nuls);
        if (stripped != "ABC" || stripped.len() !== 3) begin
            $display("FAIL string_conversions nul_stripping");
            $finish;
        end

        packed_source = "Hi";
        packed128 = packed128_t'(packed_source);
        packed136 = packed136_t'(packed_source);
        if (packed128 !== {{112{1'b0}}, 16'h4869} ||
            packed136 !== {{120{1'b0}}, 16'h4869}) begin
            $display("FAIL string_conversions packed_casts");
            $finish;
        end

        original = "copy";
        copied = original;
        copied.putc(0, 8'h43);
        if (original != "copy" || copied != "Copy") begin
            $display("FAIL string_conversions copy_independence");
            $finish;
        end

        shown = "golden";
        $display("STRING-DISPLAY %s", shown);
        $display("PASS string_conversions");
        $finish;
    end
endmodule
