// llg-test-fixture: tests/fixtures/sim/static_review/activation_jumps.sv
// Static-review regression source; not executed in the review session.
module tb;
    integer i, count, result;
    function automatic integer leave_block(input integer value);
        begin : early
            if (value != 0) return value;
        end
        return 0;
    endfunction
    initial begin
        count = 0;
        for (i = 0; i < 4; i = i + 1) begin : iteration
            if (i == 0) continue;
            if (i == 2) break;
            count = count + 1;
        end
        result = leave_block(7);
        result = result + leave_block(0);
        $display("count=%0d result=%0d", count, result);
        $finish(0);
    end
endmodule
