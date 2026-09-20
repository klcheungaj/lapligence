// Bound default patterns must retain declaration order and executable values.
module tb;
    logic [7:0] descending [3:1][-1:0];
    logic [7:0] ascending [-2:0][1:0];
    // The untyped nested default binds to the packed byte's bits, not a row.
    logic default_bit;
    initial begin
        default_bit = 1'b1;
        descending = '{3: '{8'h31, 8'h32}, default: '{default: default_bit}};
        default_bit = 1'b0;
        ascending = '{0: '{8'h01, 8'h02}, default: '{default: default_bit}};
        if (descending[3][-1] !== 8'h31 || descending[3][0] !== 8'h32)
            $fatal(1, "descending explicit key moved to the wrong row");
        if (descending[2][-1] !== 8'hff || descending[2][0] !== 8'hff ||
            descending[1][-1] !== 8'hff || descending[1][0] !== 8'hff)
            $fatal(1, "descending nested default was not captured");
        if (ascending[0][1] !== 8'h01 || ascending[0][0] !== 8'h02)
            $fatal(1, "ascending explicit key moved to the wrong row");
        if (ascending[-2][1] !== 8'h00 || ascending[-2][0] !== 8'h00 ||
            ascending[-1][1] !== 8'h00 || ascending[-1][0] !== 8'h00)
            $fatal(1, "ascending nested default was not captured");
        $display("nested_defaults passed");
        $finish(0);
    end
endmodule
