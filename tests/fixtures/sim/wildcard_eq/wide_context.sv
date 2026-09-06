module tb;
    logic [7:0] dividend;
    logic [7:0] divisor;
    logic [127:0] rhs;
    logic result;

    initial begin
        dividend = 8'd250;
        divisor = 8'd5;
        rhs = 128'd50;
        result = (dividend / divisor) ==? rhs;
        if (result !== 1'b1) begin
            $display("FAIL wildcard_wide_context equal");
            $finish;
        end

        rhs[100] = 1'b1;
        result = (dividend / divisor) ==? rhs;
        if (result !== 1'b0) begin
            $display("FAIL wildcard_wide_context high_mismatch");
            $finish;
        end

        rhs[100] = 1'bx;
        result = (dividend / divisor) ==? rhs;
        if (result !== 1'b1) begin
            $display("FAIL wildcard_wide_context high_wildcard");
            $finish;
        end

        dividend = 8'bx;
        rhs = '0;
        result = (dividend / divisor) ==? rhs;
        if (result !== 1'bx) begin
            $display("FAIL wildcard_wide_context unknown_lhs");
            $finish;
        end

        rhs = 'x;
        result = (dividend / divisor) ==? rhs;
        if (result !== 1'b1) begin
            $display("FAIL wildcard_wide_context all_wildcards");
            $finish;
        end

        $display("PASS wildcard_wide_context");
        $finish;
    end
endmodule
