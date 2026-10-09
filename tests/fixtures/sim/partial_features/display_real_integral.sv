// llg-test-fixture: tests/fixtures/sim/partial_features/display_real_integral.sv
module tb;
    initial begin
        $display("%d", 1.0);
        $finish(0);
    end
endmodule
