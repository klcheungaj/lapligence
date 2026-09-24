// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_source_return_initializers.sv
module tb;
    // return_source is written with WK=procedural_blocking and
    // LV=whole_object, then read by CO=function_return_statement in HC=subroutine;
    // its caller runs in PC=initial.
    function automatic logic [7:0] return_written_local();
        logic [7:0] return_source;
        return_source = 8'h5a;
        return return_source;
    endfunction

    function automatic logic [7:0] written_helper(input logic [7:0] seed);
        logic [7:0] helper_source;
        helper_source = seed;
        return helper_source;
    endfunction

    // helper_source is written with WK=procedural_blocking and LV=whole_object.
    // Its return read has CO=function_return_statement, HC=subroutine, PC=none.
    // The module consumer has CO=declaration_initializer, IN=runtime_declaration,
    // HC=module, PC=none. The constant consumer has
    // CO=declaration_initializer, IN=constant_declaration, HC=module, PC=none.
    logic [7:0] module_initializer = written_helper(8'h5a);
    localparam logic [7:0] constant_initializer = written_helper(8'h5a);

    logic [7:0] module_source;
    logic [7:0] initializer_source;

    initial begin : check
        logic [7:0] local_return_value;
        local_return_value = return_written_local();

        module_source = 8'h5a;
        initializer_source = 8'h5a;
        begin : nested
            // The two initializer reads share the same module-level write/use
            // context: CO=declaration_initializer, IN=automatic_local,
            // HC=module, PC=initial, WK=procedural_blocking, LV=whole_object.
            automatic logic [7:0] local_initializer = module_source;
            automatic logic [7:0] initializer_copy = initializer_source;

            if (local_initializer !== 8'h5a || initializer_copy !== 8'h5a)
                $fatal(1, "written-source initializer mismatch");

            $display("local_initializer=%02h", local_initializer);
            $display("initializer_source=%02h", initializer_copy);
        end

        begin : static_helper_use
            // CO=declaration_initializer, IN=static_local, HC=module, PC=none;
            // helper_source's same-focal writer is procedural_blocking.
            static logic [7:0] static_initializer = written_helper(8'h5a);
            if (static_initializer !== 8'h5a)
                $fatal(1, "static written-source initializer mismatch");
            $display("static_initializer=%02h", static_initializer);
        end

        if (local_return_value !== 8'h5a ||
            module_initializer !== 8'h5a ||
            constant_initializer !== 8'h5a)
            $fatal(1, "written-source return or initializer mismatch");

        $display("local_return=%02h", local_return_value);
        $display("module_initializer=%02h", module_initializer);
        $display("constant_initializer=%02h", constant_initializer);
        $finish(0);
    end
endmodule
