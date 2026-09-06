// IEEE 1364-2001 2.6: a string is an unsigned integral constant with eight
// bits per character. This 129-character literal is therefore 1032 bits.
module tb;
    reg [1031:0] value;
    reg [1031:0] expected;

    initial begin
        value = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        expected = {129{8'h41}};
        if (value !== expected || value[1031:1024] !== 8'h41 ||
            value[519:512] !== 8'h41 || value[7:0] !== 8'h41) begin
            $display("FAIL wide_string_1032");
            $finish;
        end
        $display("PASS wide_string_1032");
        $finish;
    end
endmodule
