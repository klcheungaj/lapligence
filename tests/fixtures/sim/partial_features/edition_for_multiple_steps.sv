// llg-test-fixture: tests/fixtures/sim/partial_features/edition_for_multiple_steps.sv
// The only later construct here is a comma-separated for step list.
module tb;
    integer i, j;
    initial begin
        j = 0;
        for (i = 0; i < 3; i = i + 1, j = j + 2) begin end
        $display("for_list=%0d/%0d", i, j);
        $finish(0);
    end
endmodule
