// A zero repeat count skips the event control but still schedules the named
// event trigger in the current time slot's NBA region.
module tb;
    event source;
    event target_zero, target_negative, target_unknown;
    integer unknown_count;

    initial begin
        unknown_count = 'x;
        ->> repeat (0) @source target_zero;
        ->> repeat (-1) @source target_negative;
        ->> repeat (unknown_count) @source target_unknown;
    end

    initial begin @target_zero;
        $display("CHECK: zero repeat time=%0t", $time);
    end
    initial begin @target_negative;
        $display("CHECK: negative repeat time=%0t", $time);
    end
    initial begin @target_unknown;
        $display("CHECK: unknown repeat time=%0t", $time);
    end
    initial #1 $finish(0);
endmodule
