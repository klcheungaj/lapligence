// Each operand is below the exclusive 2^20-bit safeguard, but the
// concatenation is exactly 2^20 bits and must be rejected as an intermediate.
module tb #(parameter WIDTH = 524288);
    logic [WIDTH-1:0] left;
    logic [WIDTH-1:0] right;
    logic [15:0] sink;

    initial begin
        left = '0;
        right = '0;
        sink = {left, right};
        $finish;
    end
endmodule
