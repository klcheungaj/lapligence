// llg-test-fixture: tests/fixtures/sim/nonconvergence/finite_zero_time_loop.sv
// IEEE 1364-2001 §9.9.2: finite procedural computation may complete at one time.
module tb;
    integer i;

    initial begin
        for (i = 0; i < 200000; i = i + 1) begin
        end
        $display("finite=%0d", i);
        $finish(0);
    end
endmodule
