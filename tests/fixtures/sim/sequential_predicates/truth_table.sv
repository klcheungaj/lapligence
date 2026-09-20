module tb;
    logic a, b, c;
    logic [7:0] result, expected;
    int true_count, false_count, unknown_count, selected;
    function automatic logic state(input int code);
        case (code)
            0: return 1'b0;
            1: return 1'b1;
            2: return 1'bx;
            default: return 1'bz;
        endcase
    endfunction
    initial begin
        true_count = 0; false_count = 0; unknown_count = 0;
        for (int i = 0; i < 64; i++) begin
            a = state(i / 16); b = state((i / 4) % 4); c = state(i % 4);
            result = a &&& b &&& c ? 8'ha5 : 8'ha6;
            if (a === 1'b1 && b === 1'b1 && c === 1'b1) begin
                expected = 8'ha5;
                true_count++;
            end else if (a === 1'b0 ||
                         (a === 1'b1 && b === 1'b0) ||
                         (a === 1'b1 && b === 1'b1 && c === 1'b0)) begin
                expected = 8'ha6;
                false_count++;
            end else begin
                expected = 8'b101001xx;
                unknown_count++;
            end
            if (result !== expected) $fatal(1, "ternary truth table at %0d", i);
            if (a &&& b &&& c) selected = 1;
            else selected = 2;
            if (selected != ((expected === 8'ha5) ? 1 : 2))
                $fatal(1, "if truth table at %0d", i);
        end
        $display("truth_table=64 true=%0d false=%0d unknown=%0d",
                 true_count, false_count, unknown_count);
        $finish(0);
    end
endmodule
