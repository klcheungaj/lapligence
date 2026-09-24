// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/subroutine_event_signal_matrix.sv
module tb;
    bit module_source;
    bit module_seen;
    bit formal_seen;

    // Focal vector: integral_bit_logic, direct_projection, event_expression,
    // whole_object, module_package, none, subroutine, local, none, none, none,
    // procedural_blocking, initial.
    task automatic wait_module_source();
        @(module_source);
        module_seen = 1'b1;
    endtask

    // Focal vector: integral_bit_logic, direct_projection, event_expression,
    // whole_object, formal, none, subroutine, local, none, none, none,
    // procedural_blocking, initial.
    task wait_formal(input bit source, input bit observer);
        if (observer) begin
            @(source);
            formal_seen = 1'b1;
        end else begin
            #1 source = 1'b1;
        end
    endtask

    initial begin
        module_source = 1'b0;
        module_seen = 1'b0;
        formal_seen = 1'b0;
        fork
            begin
                wait_module_source();
            end
            begin
                #1 module_source = 1'b1;
            end
        join
        if (!module_seen)
            $fatal(1, "subroutine module event expression missed a transition");

        fork
            begin
                wait_formal(1'b0, 1'b1);
            end
            begin
                wait_formal(1'b0, 1'b0);
            end
        join
        if (!formal_seen)
            $fatal(1, "subroutine event expression missed a transition");
        $display("module=%0d formal=%0d", module_seen, formal_seen);
        $finish(0);
    end
endmodule
