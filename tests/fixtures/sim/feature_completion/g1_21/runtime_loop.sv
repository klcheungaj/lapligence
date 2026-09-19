// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_21/runtime_loop.sv
// G1-21 loop_not_synthesis_proven: a legal, runtime-terminating loop must
// simulate even though its bound is not statically provable.
module tb;
    integer i;
    integer sum;
    integer bound;

    function automatic integer next_bound(input integer current);
        next_bound = current + 1;
    endfunction

    initial begin
        i = 0;
        sum = 0;
        bound = next_bound(6);
        while (i < bound) begin
            sum = sum + i;
            i = i + 1;
            if (sum > 10) break;
        end
        $display("sum=%0d i=%0d", sum, i);
        $finish(0);
    end
endmodule
