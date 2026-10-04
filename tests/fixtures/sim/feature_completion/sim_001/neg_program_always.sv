// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/neg_program_always.sv
// Language-illegal neighbour of the reactive set (IEEE 1800-2009 §24.3):
// a program may contain initial procedures but no always procedure.
program p;
    int n = 0;
    always #1 n++;
endprogram

module tb;
    p p0();
endmodule
