// IEEE 1800-2009 6.24.1: bit'(four_state_value) is a sized one-bit,
// two-state cast; X and Z map to zero.
module tb #(parameter WIDTH = 128);
    logic scalar_x;
    logic scalar_z;
    logic scalar_one;
    // Four-state observers ensure conversion happens in bit'(), not in the
    // destination assignment.
    logic from_x;
    logic from_z;
    logic from_one;
    integer failed;

    initial begin
        scalar_x = 1'bx;
        scalar_z = 1'bz;
        scalar_one = 1'b1;
        #1;
        from_x = bit'(scalar_x);
        from_z = bit'(scalar_z);
        from_one = bit'(scalar_one);
        #1;

        failed = 0;
        if (from_x !== 1'b0) begin
            $display("FAIL scalar-x-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && from_z !== 1'b0) begin
            $display("FAIL scalar-z-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && from_one !== 1'b1) begin
            $display("FAIL scalar-known-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS two_state_scalar_cast WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
