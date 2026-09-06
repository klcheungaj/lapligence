// IEEE 1800-2009 11.4.13: integral set elements use asymmetric wildcard
// equality, ranges are inclusive, and mixed signedness follows equality rules.
module tb;
    logic [5:0] value;
    logic [2:0] unknown_value;
    logic signed [3:0] signed_nibble;
    logic result;

    initial begin
        value = 6'd5;
        result = value inside {6'd3, 6'd5, 6'd7};
        if (result !== 1'b1) begin
            $display("FAIL inside_membership scalar_hit");
            $finish;
        end
        value = 6'd6;
        result = value inside {6'd3, 6'd5, 6'd7};
        if (result !== 1'b0) begin
            $display("FAIL inside_membership scalar_miss");
            $finish;
        end

        value = 6'd16;
        result = value inside {[6'd16:6'd23], [6'd32:6'd47]};
        if (result !== 1'b1) begin
            $display("FAIL inside_membership range_low_bound");
            $finish;
        end
        value = 6'd23;
        result = value inside {[6'd16:6'd23]};
        if (result !== 1'b1) begin
            $display("FAIL inside_membership range_high_bound");
            $finish;
        end
        value = 6'd6;
        result = value inside {[6'd7:6'd4]};
        if (result !== 1'b0) begin
            $display("FAIL inside_membership empty_range");
            $finish;
        end

        unknown_value = 3'bz11;
        result = unknown_value inside {3'b1?1, 3'b011};
        if (result !== 1'bx) begin
            $display("FAIL inside_membership unknown_lhs");
            $finish;
        end
        unknown_value = 3'bx01;
        result = unknown_value inside {3'b?01};
        if (result !== 1'b1) begin
            $display("FAIL inside_membership wildcard_hit");
            $finish;
        end
        result = unknown_value inside {3'b001};
        if (result !== 1'bx) begin
            $display("FAIL inside_membership wildcard_unknown");
            $finish;
        end
        result = unknown_value inside {3'b001, 3'b?01};
        if (result !== 1'b1) begin
            $display("FAIL inside_membership later_definite_hit");
            $finish;
        end

        signed_nibble = -4'sd1;
        result = signed_nibble inside {8'shff};
        if (result !== 1'b1) begin
            $display("FAIL inside_membership signed_extension");
            $finish;
        end
        result = signed_nibble inside {8'hff};
        if (result !== 1'b0) begin
            $display("FAIL inside_membership unsigned_extension");
            $finish;
        end

        $display("PASS inside_membership");
        $finish;
    end
endmodule
