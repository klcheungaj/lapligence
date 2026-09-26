// llg-test-fixture: tests/fixtures/sim/loops/syn_037_task_step_rejected.sv
// A task is not the function_subroutine_call admitted as a for step.
module tb;
    integer i;
    task automatic advance;
        i = i + 1;
    endtask
    initial for (i = 0; i < 2; advance()) begin end
endmodule
