// llg-test-fixture: tests/fixtures/sim/nonconvergence/region_pass_boundary.sv
// IEEE 1364-2001 §9.9.2 permits a finite zero-delay yield. The simulator
// policy counts the initial and resumed process dispatch as two region passes.
module tb;
    initial begin
        #0;
        $display("PASS region passes");
        $finish(0);
    end
endmodule
