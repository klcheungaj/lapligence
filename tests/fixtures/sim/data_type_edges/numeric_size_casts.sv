// IEEE 1800-2009 6.24.1: a positive integral size before the apostrophe
// sets the result width while preserving the operand's signedness. Whitespace
// and newlines between the cast tokens do not change that meaning.
module tb #(parameter WIDTH = 128);
    logic signed [31:0] signed_source;
    logic signed [WIDTH-1:0] fixed_single_line;
    logic signed [WIDTH-1:0] fixed_multiline;
    logic signed [WIDTH:0] parameter_single_line;
    logic signed [WIDTH:0] parameter_multiline;
    logic signed [WIDTH-1:0] expected_fixed;
    logic signed [WIDTH:0] expected_parameter;
    integer failed;

    initial begin
        failed = 0;
        signed_source = 32'h0001_8001;
        fixed_single_line = 16'(signed_source);
        fixed_multiline =
            16'
            (
                signed_source
            );
        expected_fixed = '1;
        expected_fixed[15:0] = 16'h8001;
        if (fixed_single_line !== expected_fixed ||
            fixed_multiline !== expected_fixed) begin
            $display("FAIL fixed-numeric-size-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_source = -2;
        parameter_single_line = WIDTH'(signed_source);
        parameter_multiline =
            WIDTH'
            (
                signed_source
            );
        expected_parameter = '1;
        expected_parameter[0] = 1'b0;
        if (!failed &&
            (parameter_single_line !== expected_parameter ||
             parameter_multiline !== expected_parameter)) begin
            $display("FAIL parameter-size-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS numeric_size_casts WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
