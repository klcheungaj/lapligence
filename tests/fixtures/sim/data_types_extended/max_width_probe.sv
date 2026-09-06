// The simulator's published packed-width safeguard is exclusive: WIDTH below
// 2^20 remains admissible. This probe avoids WIDTH-proportional HDL loops.
module tb #(parameter WIDTH = 1048575);
    logic [WIDTH-1:0] value;
    logic [WIDTH-1:0] result;
    logic [WIDTH-1:0] expected;
    integer failed;

    initial begin
        failed = 0;
        value = '0;
        value[WIDTH-1] = 1'b1;
        value[64] = 1'b1;
        result = value >> (WIDTH-1);
        expected = '0;
        expected[0] = 1'b1;
        if (result !== expected) begin
            $display("FAIL maximum-minus-one-shift WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && (|value) !== 1'b1) begin
            $display("FAIL maximum-minus-one-reduction WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed) $display("PASS max_width_probe WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
