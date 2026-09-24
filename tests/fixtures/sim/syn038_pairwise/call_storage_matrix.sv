// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/call_storage_matrix.sv
// Each focal path is a call-site actual: SL describes that actual's storage,
// while FM describes the distinct receiving formal. Module, interface, and
// hierarchical routes are kept at separate call statements.
module route_sink;
    task automatic update_hier_inout(inout logic [7:0] value);
        value = value + 8'h03;
    endtask

    function automatic logic [7:0] read_hier_const(const ref logic [7:0] value);
        return value ^ 8'h0f;
    endfunction
endmodule

interface route_if;
    logic [7:0] iface_input;
    logic [7:0] iface_output;
    logic [7:0] iface_inout;
    logic [7:0] iface_ref;
    logic [7:0] iface_const;
    logic [7:0] iface_input_result;
    logic [7:0] iface_const_result;

    function automatic logic [7:0] use_input(input logic [7:0] value);
        return value ^ 8'h01;
    endfunction

    function automatic logic [7:0] use_const(const ref logic [7:0] value);
        return value ^ 8'h02;
    endfunction

    task automatic use_output(output logic [7:0] value);
        value = 8'h31;
    endtask

    task automatic use_inout(inout logic [7:0] value);
        value = value + 8'h04;
    endtask

    task automatic use_ref(ref logic [7:0] value);
        value = value ^ 8'h08;
    endtask

    initial begin
        iface_input = 8'h10;
        iface_output = 8'h00;
        iface_inout = 8'h20;
        iface_ref = 8'h30;
        iface_const = 8'h40;
        iface_input_result = use_input(iface_input);
        if (iface_input_result !== 8'h11)
            $fatal(1, "interface input function actual mismatch");
        use_output(iface_output);
        if (iface_output !== 8'h31)
            $fatal(1, "interface output actual mismatch");
        use_inout(iface_inout);
        if (iface_inout !== 8'h24)
            $fatal(1, "interface inout actual mismatch");
        use_ref(iface_ref);
        if (iface_ref !== 8'h38)
            $fatal(1, "interface ref actual mismatch");
        iface_const_result = use_const(iface_const);
        if (iface_const_result !== 8'h42)
            $fatal(1, "interface const-ref function actual mismatch");
    end
endinterface

module tb;
    logic [7:0] static_input_result;
    logic [7:0] static_const_result;
    logic [7:0] automatic_input_result;
    logic [7:0] automatic_const_result;
    logic [7:0] formal_input_result;
    logic [7:0] formal_const_result;
    logic [7:0] return_const_result;
    logic [7:0] module_iface_const_result;
    logic [7:0] hier_const_value = 8'h60;
    logic [7:0] hier_const_result;
    route_if bus();
    route_sink child();

    function automatic logic [7:0] use_input(input logic [7:0] value);
        return value + 8'h01;
    endfunction

    function automatic logic [7:0] use_const(const ref logic [7:0] value);
        return value + 8'h02;
    endfunction

    task automatic use_output(output logic [7:0] value);
        value = 8'h41;
    endtask

    task automatic use_inout(inout logic [7:0] value);
        value = value + 8'h03;
    endtask

    task automatic use_ref(ref logic [7:0] value);
        value = value ^ 8'h0c;
    endtask

    task automatic exercise_static();
        static logic [7:0] value_input;
        static logic [7:0] value_output;
        static logic [7:0] value_inout;
        static logic [7:0] value_ref;
        static logic [7:0] value_const;
        value_input = 8'h11;
        static_input_result = use_input(value_input);
        if (static_input_result !== 8'h12)
            $fatal(1, "static local input function actual mismatch");
        value_output = 8'h00;
        use_output(value_output);
        if (value_output !== 8'h41)
            $fatal(1, "static local output actual mismatch");
        value_inout = 8'h21;
        use_inout(value_inout);
        if (value_inout !== 8'h24)
            $fatal(1, "static local inout actual mismatch");
        value_ref = 8'h31;
        use_ref(value_ref);
        if (value_ref !== 8'h3d)
            $fatal(1, "static local ref actual mismatch");
        value_const = 8'h41;
        static_const_result = use_const(value_const);
        if (static_const_result !== 8'h43)
            $fatal(1, "static local const-ref function actual mismatch");
    endtask

    task automatic exercise_automatic();
        logic [7:0] value_input;
        logic [7:0] value_output;
        logic [7:0] value_inout;
        logic [7:0] value_const;
        value_input = 8'h12;
        automatic_input_result = use_input(value_input);
        if (automatic_input_result !== 8'h13)
            $fatal(1, "automatic local input function actual mismatch");
        value_output = 8'h00;
        use_output(value_output);
        if (value_output !== 8'h41)
            $fatal(1, "automatic local output actual mismatch");
        value_inout = 8'h22;
        use_inout(value_inout);
        if (value_inout !== 8'h25)
            $fatal(1, "automatic local inout actual mismatch");
        value_const = 8'h42;
        automatic_const_result = use_const(value_const);
        if (automatic_const_result !== 8'h44)
            $fatal(1, "automatic local const-ref function actual mismatch");
    endtask

    task automatic exercise_formal(ref logic [7:0] value);
        formal_input_result = use_input(value);
        if (formal_input_result !== 8'h25)
            $fatal(1, "formal input function actual mismatch");
        use_inout(value);
        if (value !== 8'h27)
            $fatal(1, "formal inout actual mismatch");
        use_ref(value);
        if (value !== 8'h2b)
            $fatal(1, "formal ref actual mismatch");
        formal_const_result = use_const(value);
        if (formal_const_result !== 8'h2d)
            $fatal(1, "formal const-ref function actual mismatch");
    endtask

    function automatic logic [7:0] exercise_return_slot();
        exercise_return_slot = 8'h53;
        exercise_return_slot = use_const(exercise_return_slot);
        if (exercise_return_slot !== 8'h55)
            $fatal(1, "function return slot const-ref actual mismatch");
    endfunction

    initial begin
        logic [7:0] formal_value;
        logic [7:0] iface_actual;
        #1;
        exercise_static();
        exercise_automatic();
        formal_value = 8'h24;
        exercise_formal(formal_value);
        return_const_result = exercise_return_slot();

        bus.iface_output = 8'h00;
        bus.iface_inout = 8'h10;
        bus.iface_ref = 8'h20;
        // The unqualified helper callee leaves each bus member as the route.
        use_output(bus.iface_output);
        if (bus.iface_output !== 8'h41)
            $fatal(1, "module interface output actual mismatch");
        use_inout(bus.iface_inout);
        if (bus.iface_inout !== 8'h13)
            $fatal(1, "module interface inout actual mismatch");
        use_ref(bus.iface_ref);
        if (bus.iface_ref !== 8'h2c)
            $fatal(1, "module interface ref actual mismatch");
        module_iface_const_result = use_const(bus.iface_input);
        if (module_iface_const_result !== 8'h12)
            $fatal(1, "module interface const-ref actual mismatch");

        iface_actual = 8'h30;
        // The qualified child callee selects HR=hierarchical_identifier;
        // iface_actual remains the focal module-storage actual.
        child.update_hier_inout(iface_actual);
        if (iface_actual !== 8'h33)
            $fatal(1, "hierarchical inout actual mismatch");
        hier_const_result = child.read_hier_const(hier_const_value);
        if (hier_const_result !== 8'h6f)
            $fatal(1, "hierarchical const-ref actual mismatch");

        $display("calls=%h,%h,%h,%h,%h,%h,%h iface=%h,%h,%h,%h,%h,%h hier=%h,%h",
            static_input_result, static_const_result,
            automatic_input_result, automatic_const_result,
            formal_input_result, formal_const_result, return_const_result,
            bus.iface_input_result, bus.iface_output, bus.iface_inout, bus.iface_ref,
            bus.iface_const_result, module_iface_const_result,
            iface_actual, hier_const_result);
        $finish(0);
    end
endmodule
