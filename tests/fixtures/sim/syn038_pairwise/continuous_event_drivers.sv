// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/continuous_event_drivers.sv
module tb;
    logic wire_seed;
    wire wire_source;
    logic logic_seed;
    logic logic_source;

    bit observers_armed;
    bit wire_initial_seen;
    bit logic_initial_seen;
    bit wire_always_seen;
    bit logic_always_seen;
    bit wire_task_seen;
    bit logic_task_seen;
    logic wire_always_sample;
    logic logic_always_sample;
    logic wire_ff_seen;
    logic logic_ff_seen;
    logic wire_ff_sample;
    logic logic_ff_sample;
    logic wire_task_sample;
    logic logic_task_sample;
    logic wire_comb_sample;
    logic logic_comb_sample;
    logic wire_latch_sample;
    logic logic_latch_sample;
    bit wire_latch_enable;
    bit logic_latch_enable;

    // Each observer reads this focal source with a separate event expression.
    // In (TY, OP, CO, LV, SL, FM, HC, HR, CP, CT, IN, WK, PC) order, the
    // module-scope observers use integral_bit_logic, direct_projection,
    // event_expression, whole_object, module_package, none, module, local, none,
    // none, none, continuous_net, and their own process kind. The task
    // observer has the same vector with HC=subroutine and PC=initial.
    assign wire_source = wire_seed;

    // The logic_source observers use the same vectors with WK=continuous_variable.
    assign logic_source = logic_seed;

    // Focal RHS vector: integral_bit_logic, direct_projection, assignment_rhs,
    // whole_object, module_package, none, module, local, none, none, none,
    // continuous_net, always_comb.
    always_comb wire_comb_sample = wire_source;

    // Focal RHS vector: integral_bit_logic, direct_projection, assignment_rhs,
    // whole_object, module_package, none, module, local, none, none, none,
    // continuous_variable, always_comb.
    always_comb logic_comb_sample = logic_source;

    // Focal RHS vector: integral_bit_logic, direct_projection, assignment_rhs,
    // whole_object, module_package, none, module, local, none, none, none,
    // continuous_net, always_latch.
    always_latch begin
        if (wire_latch_enable)
            wire_latch_sample = wire_source;
    end

    // Focal RHS vector: integral_bit_logic, direct_projection, assignment_rhs,
    // whole_object, module_package, none, module, local, none, none, none,
    // continuous_variable, always_latch.
    always_latch begin
        if (logic_latch_enable)
            logic_latch_sample = logic_source;
    end

    always @(wire_source) begin
        if (observers_armed && !wire_always_seen) begin
            wire_always_sample = wire_source;
            wire_always_seen = 1'b1;
        end
    end

    always @(logic_source) begin
        if (observers_armed && !logic_always_seen) begin
            logic_always_sample = logic_source;
            logic_always_seen = 1'b1;
        end
    end

    always_ff @(posedge wire_source) begin
        wire_ff_seen <= 1'b1;
        wire_ff_sample <= wire_source;
    end

    always_ff @(posedge logic_source) begin
        logic_ff_seen <= 1'b1;
        logic_ff_sample <= logic_source;
    end

    task automatic wait_wire_source();
        @(wire_source);
        wire_task_sample = wire_source;
        wire_task_seen = 1'b1;
    endtask

    task automatic wait_logic_source();
        @(logic_source);
        logic_task_sample = logic_source;
        logic_task_seen = 1'b1;
    endtask

    initial begin
        wire_seed = 1'b0;
        logic_seed = 1'b0;
        observers_armed = 1'b0;
        wire_initial_seen = 1'b0;
        logic_initial_seen = 1'b0;
        wire_always_seen = 1'b0;
        logic_always_seen = 1'b0;
        wire_task_seen = 1'b0;
        logic_task_seen = 1'b0;
        wire_latch_enable = 1'b1;
        logic_latch_enable = 1'b1;
        #1;
        if (wire_source !== 1'b0 || logic_source !== 1'b0
            || wire_comb_sample !== 1'b0 || logic_comb_sample !== 1'b0
            || wire_latch_sample !== 1'b0 || logic_latch_sample !== 1'b0)
            $fatal(1, "continuous source or process-consumer baseline mismatch");
        wire_latch_enable = 1'b0;
        logic_latch_enable = 1'b0;
        observers_armed = 1'b1;

        fork
            begin
                @(wire_source);
                if (wire_source !== 1'b1)
                    $fatal(1, "wire initial event readback mismatch");
                wire_initial_seen = 1'b1;
            end
            begin
                @(logic_source);
                if (logic_source !== 1'b1)
                    $fatal(1, "logic initial event readback mismatch");
                logic_initial_seen = 1'b1;
            end
            begin
                wait_wire_source();
            end
            begin
                wait_logic_source();
            end
            begin
                #1 wire_seed = 1'b1;
            end
            begin
                #1 logic_seed = 1'b1;
            end
        join

        #1;
        if (!wire_initial_seen || !logic_initial_seen
            || !wire_always_seen || !logic_always_seen
            || !wire_ff_seen || !logic_ff_seen
            || !wire_task_seen || !logic_task_seen)
            $fatal(1, "a continuous event observer missed the transition");
        if (wire_always_sample !== 1'b1 || logic_always_sample !== 1'b1
            || wire_ff_sample !== 1'b1 || logic_ff_sample !== 1'b1
            || wire_task_sample !== 1'b1 || logic_task_sample !== 1'b1
            || wire_comb_sample !== 1'b1 || logic_comb_sample !== 1'b1
            || wire_latch_sample !== 1'b0 || logic_latch_sample !== 1'b0)
            $fatal(1, "a continuous event observer read back the wrong value");
        wire_latch_enable = 1'b1;
        logic_latch_enable = 1'b1;
        #1;
        if (wire_latch_sample !== 1'b1 || logic_latch_sample !== 1'b1)
            $fatal(1, "latch consumers failed to capture the updated source");

        wire_latch_enable = 1'b0;
        logic_latch_enable = 1'b0;
        wire_seed = 1'b0;
        logic_seed = 1'b0;
        #1;
        if (wire_source !== 1'b0 || logic_source !== 1'b0
            || wire_comb_sample !== 1'b0 || logic_comb_sample !== 1'b0
            || wire_latch_sample !== 1'b1 || logic_latch_sample !== 1'b1)
            $fatal(1, "combinational update or latch hold mismatch");
        $display("wire=%0d,%0d,%0d,%0d,%0d,%0d logic=%0d,%0d,%0d,%0d,%0d,%0d",
                 wire_initial_seen, wire_always_seen, wire_ff_seen, wire_task_seen,
                 wire_comb_sample, wire_latch_sample,
                 logic_initial_seen, logic_always_seen, logic_ff_seen, logic_task_seen,
                 logic_comb_sample, logic_latch_sample);
        $finish(0);
    end
endmodule
