// IEEE 1800-2009 6.9 and 11.6: packed integral vectors are not restricted to
// 1024 bits, and arithmetic preserves the expression's context-determined width.
module tb;
    localparam WIDTH = 2048;
    logic [WIDTH-1:0] value;
    logic [WIDTH-1:0] result;
    logic [WIDTH-1:0] expected;

    initial begin
        value = '0;
        value[WIDTH-1] = 1'b1;
        value[WIDTH/2] = 1'b1;
        value[63:0] = 64'hffff_ffff_ffff_ffff;
        result = value + 1'b1;
        expected = '0;
        expected[WIDTH-1] = 1'b1;
        expected[WIDTH/2] = 1'b1;
        expected[64] = 1'b1;
        if (result !== expected) begin
            $display("FAIL wide_signal_2048");
            $finish;
        end
        $display("PASS wide_signal_2048");
        $finish;
    end
endmodule
