// IEEE 1800-2009 11.4.3 and 11.6: equality supplies its maximum operand width
// to a context-determined arithmetic operand; division is not limited to 64 bits.
module tb;
    logic [7:0] dividend;
    logic [7:0] divisor;
    logic [127:0] comparison_rhs;
    logic result;

    initial begin
        dividend = 8'd200;
        divisor = 8'd3;
        comparison_rhs = 128'd66;
        #1;
        result = (dividend / divisor) == comparison_rhs;
        if (result !== 1'b1) begin
            $display("FAIL nested_wide_div_context true-comparison");
            $finish;
        end

        comparison_rhs[100] = 1'b1;
        #1;
        result = (dividend / divisor) == comparison_rhs;
        if (result !== 1'b0) begin
            $display("FAIL nested_wide_div_context high-mismatch");
            $finish;
        end

        $display("PASS nested_wide_div_context");
        $finish;
    end
endmodule
