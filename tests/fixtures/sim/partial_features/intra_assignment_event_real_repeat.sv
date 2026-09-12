// llg-test-fixture: tests/fixtures/sim/partial_features/intra_assignment_event_real_repeat.sv
module tb;
    reg a;
    reg b;
    reg clk;
    real count;

    initial begin
        clk = 1'b0;
        count = 2.0;
        a = repeat (count) @(posedge clk) b;
    end
endmodule
