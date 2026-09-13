// llg-test-fixture: tests/fixtures/sim/process_semantics/timing_control.sv
// IEEE 1800-2009 §§9.2.2.2–9.2.2.3: combinational and latch processes cannot
// contain a statement that passes simulation time.
module tb;
    logic a;
    logic y;

    always_comb begin
        #1 y = a;
    end

    initial begin
        a = 1'b0;
        #2 $finish;
    end
endmodule
