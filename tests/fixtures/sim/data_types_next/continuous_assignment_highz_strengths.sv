// IEEE 1800-2009 10.3.4 and 28.11-28.12: highz0/highz1 remove the
// corresponding known drive. For an X drive, the non-highz endpoint remains
// in its ambiguous range and participates in resolution.
module tb;
    wire zero_maps_to_z;
    tri one_maps_to_z;
    wire absent_zero_allows_one;
    tri absent_one_allows_zero;
    wire highz_zero_x_with_one;
    wire highz_zero_x_with_zero;
    tri highz_one_x_with_zero;
    tri highz_one_x_with_one;

    assign (highz0, strong1) zero_maps_to_z = 1'b0;
    assign (strong0, highz1) one_maps_to_z = 1'b1;

    assign (highz0, strong1) absent_zero_allows_one = 1'b0;
    assign (weak0, weak1) absent_zero_allows_one = 1'b1;
    assign (strong0, highz1) absent_one_allows_zero = 1'b1;
    assign (weak0, weak1) absent_one_allows_zero = 1'b0;

    assign (highz0, strong1) highz_zero_x_with_one = 1'bx;
    assign (weak0, weak1) highz_zero_x_with_one = 1'b1;
    assign (highz0, strong1) highz_zero_x_with_zero = 1'bx;
    assign (weak0, weak1) highz_zero_x_with_zero = 1'b0;

    assign (strong0, highz1) highz_one_x_with_zero = 1'bx;
    assign (weak0, weak1) highz_one_x_with_zero = 1'b0;
    assign (strong0, highz1) highz_one_x_with_one = 1'bx;
    assign (weak0, weak1) highz_one_x_with_one = 1'b1;

    initial begin
        #1;
        if (zero_maps_to_z !== 1'bz || one_maps_to_z !== 1'bz ||
            absent_zero_allows_one !== 1'b1 || absent_one_allows_zero !== 1'b0 ||
            highz_zero_x_with_one !== 1'b1 ||
            highz_zero_x_with_zero !== 1'bx ||
            highz_one_x_with_zero !== 1'b0 ||
            highz_one_x_with_one !== 1'bx) begin
            $display("FAIL continuous_assignment_highz_strengths");
            $finish;
        end

        $display("PASS continuous_assignment_highz_strengths");
        $finish;
    end
endmodule
