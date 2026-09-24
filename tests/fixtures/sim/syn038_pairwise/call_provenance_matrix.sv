// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/call_provenance_matrix.sv
// Function results and fixed-array reduction receivers are kept distinct at
// each focal use; checks follow each call, reduction, initializer, or process.
package call_provenance_types;
    typedef logic [7:0] byte_t;
    typedef byte_t byte_pair_t [0:1];
    typedef bit [7:0] bit_byte_t;
    typedef bit_byte_t bit_byte_pair_t [0:1];
endpackage

interface call_provenance_if;
    import call_provenance_types::*;
    byte_pair_t lanes = '{8'd4, 8'd5};
    byte_t member_result;
endinterface

module call_provenance_child (
    input call_provenance_types::byte_t value,
    output call_provenance_types::byte_t echoed
);
    import call_provenance_types::*;
    byte_pair_t lanes = '{8'd6, 8'd7};

    assign echoed = value;

    function automatic byte_t make_byte();
        return 8'h5a;
    endfunction
endmodule

module call_provenance_override_child #(
    parameter logic [7:0] lanes [0:1] = '{8'h21, 8'h43},
    parameter int pick = 0
) (
    output logic [7:0] selected
);
    int index;

    initial begin
        index = pick;
        selected = lanes[index];
    end
endmodule

module tb;
    import call_provenance_types::*;

    function automatic int constant_width();
        return 6;
    endfunction

    byte_t function_source = 8'h12;
    byte_pair_t module_lanes = '{8'd2, 8'd3};
    localparam bit_byte_pair_t DEFAULT_LANES = '{8'd0, 8'd0};
    localparam byte_pair_t CONSTANT_LANES = '{8'd3, 8'd4};
    localparam byte_t CONSTANT_REDUCTION = CONSTANT_LANES.sum();
    typedef logic [constant_width()-1:0] function_width_t;
    typedef logic [CONSTANT_LANES.sum()-1:0] reduction_width_t;
    byte_t parameter_array_reduction = DEFAULT_LANES.sum();
    byte_t runtime_reduction = make_lanes().sum();
    byte_t runtime_function_result = constant_byte();
    byte_t overridden_left_value;
    byte_t overridden_right_value;
    wire byte_t continuous_reduction;

    call_provenance_if bus();
    byte_t port_echo;
    call_provenance_child child(
        .value(module_lanes.sum()),
        .echoed(port_echo)
    );
    call_provenance_override_child #(
        .lanes('{8'h51, 8'h62}),
        .pick(0)
    ) overridden_left(.selected(overridden_left_value));
    call_provenance_override_child #(
        .lanes('{8'h73, 8'h84}),
        .pick(1)
    ) overridden_right(.selected(overridden_right_value));

    logic selector = 1'b1;
    logic latch_gate = 1'b0;
    logic clock = 1'b0;
    byte_t function_conditional;
    logic function_equality;
    int function_cast;
    int function_pattern;
    byte_t function_return_value;
    byte_t function_task_value;
    byte_t function_formal_value;
    byte_t function_auto_value;
    byte_t function_static_value;
    byte_t hierarchy_function_value;
    byte_t interface_function_value;
    byte_t reduction_conditional;
    logic reduction_equality;
    int reduction_cast;
    int reduction_pattern;
    byte_t reduction_task_value;
    byte_t return_array_reduction;
    byte_t formal_reduction;
    byte_t static_reduction_receiver;
    byte_t automatic_reduction_receiver;
    byte_t hierarchy_reduction;
    byte_t interface_reduction;
    byte_t combinational_reduction;
    byte_t latch_function_value;
    byte_t latch_reduction_value;
    byte_t sequential_reduction;
    int function_event_count;

    function automatic byte_t constant_byte();
        return 8'h12;
    endfunction

    function automatic byte_t make_byte();
        return function_source;
    endfunction

    function automatic byte_t observe_byte(input byte_t value);
        value = value + 8'd1;
        return value;
    endfunction

    function automatic logic observe_bit(input logic value);
        return value;
    endfunction

    function automatic int observe_int(input int value);
        value = value + 1;
        return value;
    endfunction

    function automatic int sum_byte_pair(input byte_pair_t value);
        return int'(value[0]) + int'(value[1]);
    endfunction

    function automatic byte_t reduce_formal(input byte_pair_t values);
        return values.sum();
    endfunction

    function automatic byte_pair_t make_lanes();
        return '{8'd2, 8'd3};
    endfunction

    task automatic observe_task(input byte_t value);
        value = value + 8'd1;
        function_task_value = value;
    endtask

    task automatic observe_reduction_task(input byte_t value);
        value = value + 8'd1;
        reduction_task_value = value;
    endtask

    task automatic exercise_local_storage();
        automatic byte_t automatic_initialized = make_byte();
        static byte_t static_initialized = constant_byte();
        automatic byte_t automatic_total = module_lanes.sum();
        static byte_t static_total = make_lanes().sum();
        automatic byte_t automatic_target;
        static byte_t static_target;
        automatic byte_pair_t automatic_lanes;
        static byte_pair_t static_lanes;

        automatic_target = make_byte();
        static_target = make_byte();
        automatic_lanes = '{8'd1, 8'd2};
        static_lanes = '{8'd2, 8'd4};
        automatic_reduction_receiver = automatic_lanes.sum();
        static_reduction_receiver = static_lanes.sum();
        function_auto_value = automatic_initialized;
        function_static_value = static_initialized;

        if (automatic_initialized !== 8'h12 || static_initialized !== 8'h12 ||
            automatic_total !== 5 || static_total !== 5 ||
            automatic_target !== 8'h12 || static_target !== 8'h12 ||
            automatic_reduction_receiver !== 3 ||
            static_reduction_receiver !== 6)
            $fatal(1, "local source and reduction storage mismatch");
    endtask

    task automatic fill_formal(output byte_t value);
        value = make_byte();
    endtask

    assign continuous_reduction = module_lanes.sum();

    always_comb combinational_reduction = module_lanes.sum();

    always_latch begin
        if (latch_gate)
            latch_function_value = constant_byte();
    end

    always_latch begin
        if (latch_gate)
            latch_reduction_value = module_lanes.sum();
    end

    always_ff @(posedge clock)
        sequential_reduction <= module_lanes.sum();

    always @(make_byte())
        function_event_count++;

    initial begin
        byte_t output_actual;

        function_event_count = 0;
        function_conditional = observe_byte(selector ? make_byte() : 8'h44);
        if (function_conditional !== 8'h13 || function_source !== 8'h12)
            $fatal(1, "conditional function result or input copy mismatch");

        function_equality = observe_bit(make_byte() == 8'h12);
        if (function_equality !== 1'b1 || function_source !== 8'h12)
            $fatal(1, "equality function result mismatch");

        function_cast = observe_int(int'(make_byte()));
        if (function_cast !== 19 || function_source !== 8'h12)
            $fatal(1, "cast function result mismatch");

        function_pattern = sum_byte_pair('{make_byte(), 8'h03});
        if (function_pattern !== 21 || function_source !== 8'h12)
            $fatal(1, "pattern function result mismatch");

        function_return_value = return_function_result();
        if (function_return_value !== 8'h12)
            $fatal(1, "function return consumer mismatch");

        observe_task(make_byte());
        if (function_task_value !== 8'h13 || function_source !== 8'h12)
            $fatal(1, "function result task input copy mismatch");

        fill_formal(output_actual);
        function_formal_value = output_actual;
        if (function_formal_value !== 8'h12)
            $fatal(1, "function result output formal copy-out mismatch");

        exercise_local_storage();
        if (function_auto_value !== 8'h12 || function_static_value !== 8'h12)
            $fatal(1, "function result local initializer mismatch");

        hierarchy_function_value = child.make_byte();
        if (hierarchy_function_value !== 8'h5a)
            $fatal(1, "hierarchical function result mismatch");

        bus.member_result = make_byte();
        interface_function_value = bus.member_result;
        if (interface_function_value !== 8'h12)
            $fatal(1, "interface member function result mismatch");

        reduction_conditional = observe_byte(selector ? module_lanes.sum() : 8'd0);
        if (reduction_conditional !== 8'd6 || module_lanes[0] !== 8'd2 ||
            module_lanes[1] !== 8'd3)
            $fatal(1, "conditional reduction result or receiver mismatch");

        reduction_equality = observe_bit(module_lanes.sum() == 8'd5);
        if (reduction_equality !== 1'b1 || module_lanes[0] !== 8'd2 ||
            module_lanes[1] !== 8'd3)
            $fatal(1, "equality reduction result or receiver mismatch");

        reduction_cast = observe_int(int'(module_lanes.sum()));
        if (reduction_cast !== 6 || module_lanes[0] !== 8'd2 ||
            module_lanes[1] !== 8'd3)
            $fatal(1, "cast reduction result or receiver mismatch");

        reduction_pattern = sum_byte_pair(byte_pair_t'{module_lanes.sum(), 8'd7});
        if (reduction_pattern !== 12 || module_lanes[0] !== 8'd2 ||
            module_lanes[1] !== 8'd3)
            $fatal(1, "pattern reduction result or receiver mismatch");

        observe_reduction_task(module_lanes.sum());
        if (reduction_task_value !== 8'd6 || module_lanes[0] !== 8'd2 ||
            module_lanes[1] !== 8'd3)
            $fatal(1, "reduction task input copy mismatch");

        return_array_reduction = make_lanes().sum();
        if (return_array_reduction !== 8'd5)
            $fatal(1, "function-return array reduction mismatch");

        formal_reduction = reduce_formal(module_lanes);
        if (formal_reduction !== 8'd5)
            $fatal(1, "formal array reduction mismatch");

        hierarchy_reduction = child.lanes.sum();
        if (hierarchy_reduction !== 8'd13)
            $fatal(1, "hierarchical reduction receiver mismatch");

        interface_reduction = bus.lanes.sum();
        if (interface_reduction !== 8'd9)
            $fatal(1, "interface member reduction receiver mismatch");

        if (DEFAULT_LANES[0] !== 8'd0 || DEFAULT_LANES[1] !== 8'd0 ||
            parameter_array_reduction !== 8'd0 ||
            CONSTANT_REDUCTION !== 8'd7 || runtime_reduction !== 8'd5 ||
            runtime_function_result !== 8'h12 || $bits(function_width_t) !== 6 ||
            $bits(reduction_width_t) !== 7)
            $fatal(1, "constant or runtime declaration initializer mismatch");

        #1;
        if (overridden_left_value !== 8'h51 || overridden_right_value !== 8'h84)
            $fatal(1, "overridden array parameter instance values crossed or used defaults");
        if (continuous_reduction !== 8'd5 || combinational_reduction !== 8'd5)
            $fatal(1, "static or combinational reduction consumer mismatch");

        exercise_local_storage();
        latch_gate = 1'b1;
        clock = 1'b1;
        #1;
        if (latch_function_value !== 8'h12 || latch_reduction_value !== 8'd5 ||
            sequential_reduction !== 8'd5 || port_echo !== 8'd5)
            $fatal(1, "process or port reduction consumer mismatch");

        function_event_count = 0;
        function_source = 8'h13;
        #1;
        if (function_event_count !== 1)
            $fatal(1, "function event expression did not observe the source change");

        $display("calls=%h,%b,%0d,%0d,%0d task=%h,%h routes=%h,%h,%h,%h reductions=%0d,%b,%0d,%0d,%h,%h,%h,%h processes=%h,%h,%h,%0d parameter=%0d overrides=%h,%h",
            function_conditional, function_equality, function_cast, function_pattern,
            function_return_value, function_task_value, function_formal_value,
            function_auto_value, function_static_value,
            hierarchy_function_value, interface_function_value,
            reduction_conditional, reduction_equality, reduction_cast,
            reduction_pattern, formal_reduction, return_array_reduction,
            hierarchy_reduction, interface_reduction, latch_function_value,
            latch_reduction_value, sequential_reduction, function_event_count,
            parameter_array_reduction, overridden_left_value, overridden_right_value);
        $finish(0);
    end

    function automatic byte_t return_function_result();
        return make_byte();
    endfunction
endmodule
