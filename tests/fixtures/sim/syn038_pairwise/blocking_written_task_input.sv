// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/blocking_written_task_input.sv
module tb;
    logic [7:0] source;
    logic [7:0] observed;
    logic [7:0] control = 8'hc3;

    task automatic capture(input logic [7:0] value);
        observed = value;
    endtask

    initial begin
        source = 8'h00;
        observed = 8'h00;
        capture(source);
        if (source !== 8'h00 || observed !== 8'h00 || control !== 8'hc3)
            $fatal(1, "task input baseline mismatch");

        // Focal vector: integral_bit_logic, direct_projection, call_argument,
        // whole_object, module_package, input, task, module, local, none,
        // none, procedural_blocking, initial.
        source = 8'h5a;
        capture(source);
        if (source !== 8'h5a || observed !== 8'h5a || control !== 8'hc3)
            $fatal(1, "task input update mismatch");

        $display("call_source=%h/%h", source, observed);
        $finish(0);
    end
endmodule
