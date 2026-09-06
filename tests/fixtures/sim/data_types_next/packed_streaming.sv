// IEEE 1800-2009 11.4.14: >> preserves block order and ignores slice size;
// << reverses right-origin blocks without padding a short left-most block.
module tb;
    logic [7:0] octet;
    logic [5:0] six_bits;
    logic [9:0] ten_bits;
    logic [7:0] reversed_bits;
    logic [5:0] left_four_six;
    logic [5:0] right_four_six;
    logic [9:0] left_four_ten;
    logic [9:0] right_four_ten;

    initial begin
        octet = 8'b0011_0101;
        six_bits = 6'b11_0101;
        ten_bits = 10'b11_0101_0011;

        reversed_bits = {<<{octet}};
        left_four_six = {<<4{six_bits}};
        right_four_six = {>>4{six_bits}};
        left_four_ten = {<<4{ten_bits}};
        right_four_ten = {>>4{ten_bits}};

        if (reversed_bits !== 8'b1010_1100 ||
            left_four_six !== 6'b01_0111 ||
            right_four_six !== 6'b11_0101 ||
            left_four_ten !== 10'b00_1101_0111 ||
            right_four_ten !== 10'b11_0101_0011) begin
            $display("FAIL packed_streaming");
            $finish;
        end

        $display("PASS packed_streaming");
        $finish;
    end
endmodule
