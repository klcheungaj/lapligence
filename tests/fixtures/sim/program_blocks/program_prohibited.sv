// llg-test-fixture: tests/fixtures/sim/program_blocks/program_prohibited.sv
// IEEE 1800-2009 §24.3.1: a program block cannot contain an always process.
program prohibited;
    logic value;
    always @* value = 1'b1;
endprogram

module tb;
    prohibited prohibited0();
endmodule
