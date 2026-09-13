// llg-test-fixture: tests/fixtures/sim/static_review/repeat_counts.sv
// Static-review regression source; not executed in the review session.
module tb;
    integer negative_count, unknown_count, highz_count, wide_count;
    initial begin
        negative_count = 0;
        unknown_count = 0;
        highz_count = 0;
        wide_count = 0;
        repeat (32'shffff_ffff) negative_count = negative_count + 1;
        repeat (4'bx001) unknown_count = unknown_count + 1;
        repeat (4'bz001) highz_count = highz_count + 1;
        repeat (65'h1_0000000000000000) begin
            wide_count = wide_count + 1;
            break;
        end
        $display("negative=%0d unknown=%0d highz=%0d wide=%0d",
                 negative_count, unknown_count, highz_count, wide_count);
        $finish(0);
    end
endmodule
