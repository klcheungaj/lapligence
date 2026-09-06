// IEEE 1800-2009 10.3.4 and 28.12: equally strong opposite drivers
// resolve to X, while an unequal stronger driver determines the value.
// An asymmetric X spans its strength0/strength1 range; a known driver can
// eliminate the weaker opposite-value part of that range.
module tb;
    wire equal_wire;
    tri equal_tri;
    wire zero_wins;
    tri one_wins;
    wire asymmetric_zero;
    wire asymmetric_zero_x;
    tri asymmetric_one;
    tri asymmetric_one_x;

    assign (strong0, strong1) equal_wire = 1'b0;
    assign (strong0, strong1) equal_wire = 1'b1;
    assign (weak0, weak1) equal_tri = 1'b0;
    assign (weak0, weak1) equal_tri = 1'b1;

    assign (strong0, strong1) zero_wins = 1'b0;
    assign (weak0, weak1) zero_wins = 1'b1;
    assign (weak0, weak1) one_wins = 1'b0;
    assign (strong0, strong1) one_wins = 1'b1;

    assign (strong0, weak1) asymmetric_zero = 1'bx;
    assign (pull0, pull1) asymmetric_zero = 1'b0;
    assign (strong0, weak1) asymmetric_zero_x = 1'bx;
    assign (pull0, pull1) asymmetric_zero_x = 1'b1;

    assign (weak0, strong1) asymmetric_one = 1'bx;
    assign (pull0, pull1) asymmetric_one = 1'b1;
    assign (weak0, strong1) asymmetric_one_x = 1'bx;
    assign (pull0, pull1) asymmetric_one_x = 1'b0;

    initial begin
        #1;
        if (equal_wire !== 1'bx || equal_tri !== 1'bx ||
            zero_wins !== 1'b0 || one_wins !== 1'b1 ||
            asymmetric_zero !== 1'b0 || asymmetric_zero_x !== 1'bx ||
            asymmetric_one !== 1'b1 || asymmetric_one_x !== 1'bx) begin
            $display("FAIL continuous_assignment_strengths");
            $finish;
        end

        $display("PASS continuous_assignment_strengths");
        $finish;
    end
endmodule
