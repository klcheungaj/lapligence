// llg-test-fixture: tests/fixtures/sim/program_blocks/program_exit_outside.sv
// IEEE 1800-2009 §24.7: `$exit` is a program control task, not a module task.
module tb;
    initial $exit;
endmodule
