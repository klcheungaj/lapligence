// llg-test-fixture: tests/fixtures/sim/process_semantics/multiple_writer.sv
// IEEE 1800-2009 §9.2.2.2: an always_comb variable has a single procedural
// writer; this must reject independently of the optional lint pass.
module tb;
    logic a;
    logic q;

    always_comb q = a;
    always_comb q = ~a;

    initial begin
        a = 1'b0;
        #1 $finish;
    end
endmodule
