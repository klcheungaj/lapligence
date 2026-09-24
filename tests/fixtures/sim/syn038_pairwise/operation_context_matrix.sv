// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/operation_context_matrix.sv
// Source-side operations keep their declared type and use-site context through
// constant elaboration, procedural scopes, and subroutine calls.
package op_types;
    typedef logic [7:0] byte_t;
    typedef bit [7:0] two_state_t;
    typedef bit [0:0] index_bit_t;
    typedef enum logic [1:0] { COLOR_RED = 2'b01, COLOR_BLUE = 2'b10 } color_t;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    typedef union packed { logic [7:0] first; logic [7:0] second; } union_t;
    typedef union_t union_array_t [0:1];
    typedef struct { logic [7:0] code; logic [7:0] data; } record_t;
    typedef record_t records_t [0:1];
    typedef logic [15:0] bits16_t;
    typedef logic [31:0] bits32_t;
endpackage

interface op_interface;
    import op_types::*;
    bit choose_left;
    byte_t left_value, right_value;
    byte_t conditional_value;
    two_state_t cast_value;
    pair_t pattern_value;

    always_comb begin
        conditional_value = choose_left ? left_value : right_value;
        cast_value = two_state_t'(left_value);
        pattern_value = '{hi: left_value, lo: right_value};
    end
endinterface

module tb;
    import op_types::*;

    localparam byte_t CONST_LEFT = 8'h11;
    localparam byte_t CONST_RIGHT = 8'h22;
    localparam bit CONST_CHOOSE = 1'b0;
    localparam byte_t CONDITIONAL_CONSTANT = CONST_CHOOSE ? CONST_LEFT : CONST_RIGHT;
    localparam bit STATIC_INIT_SELECT = 1'b1;
    localparam byte_t STATIC_INIT_LEFT = 8'h25;
    localparam byte_t STATIC_INIT_RIGHT = 8'h34;

    // The assignment pattern is itself the constant expression used to size
    // this type; it is not a runtime localparam copy.
    typedef logic [(int'(pair_t'{hi: CONST_LEFT, lo: CONST_RIGHT}) % 4):0] pattern_width_t;

    bit choose_left;
    byte_t left_value, right_value;
    pair_t packed_left, packed_right, packed_conditional;
    logic packed_equality;
    records_t records_left, records_right;
    logic records_equality;
    bits32_t records_cast;
    record_t record_source;
    bits16_t record_cast;
    union_t union_pattern_left = union_t'(8'h12);
    union_t union_pattern_right = union_t'(8'h34);
    union_array_t union_pattern_result;

    byte_t task_conditional;
    logic task_equality;
    two_state_t task_cast;
    pair_t task_pattern;

    pair_t formal_pattern;

    byte_t constref_lanes [0:1];
    byte_t compare_left, compare_right, cast_index;
    bit pattern_index;
    byte_t constref_conditional, constref_equality, constref_cast, constref_pattern;
    byte_t constref_scalar_control, constref_scalar_observed;

    byte_t static_local_result;
    byte_t automatic_local_result;
    byte_t automatic_init_result;
    byte_t static_init_first;
    byte_t static_init_second;
    byte_t static_snapshot_first;
    byte_t static_snapshot_second;
    color_t color_function_source;
    color_t color_function_result;
    union_t union_function_source;
    union_t union_function_result;
    bit static_event_seen;

    bit [7:0] static_snapshot_source;

    op_interface bus();

    // Operator results are passed as user task actuals.
    task automatic capture_ops(
        input byte_t conditional_arg,
        input logic equality_arg,
        input two_state_t cast_arg,
        input pair_t pattern_arg
    );
        task_conditional = conditional_arg;
        task_equality = equality_arg;
        task_cast = cast_arg;
        task_pattern = pattern_arg;
    endtask

    // Address-only operators select a variable array element for the const-ref actual.
    // The selected source value itself is a direct projection.
    task automatic capture_selected(const ref byte_t selected_value, output byte_t observed);
        observed = selected_value;
    endtask

    task automatic pattern_from_formals(input byte_t pattern_hi, input byte_t pattern_lo);
        formal_pattern = '{hi: pattern_hi, lo: pattern_lo};
    endtask

    task automatic drive_static_event_source();
        static bit static_event_source;
        static_event_source = 1'b0;
        #1 static_event_source = 1'b1;
    endtask

    function automatic color_t echo_color(input color_t value);
        return value;
    endfunction

    function automatic union_t echo_union(input union_t value);
        return value;
    endfunction

    function automatic byte_t select_static_local(input bit select_left);
        static byte_t static_left;
        static byte_t static_right;
        static byte_t static_result;
        static_left = 8'h25;
        static_right = 8'h34;
        static_result = select_left ? static_left : static_right;
        return static_result;
    endfunction

    function automatic byte_t select_automatic_local(input bit select_left);
        automatic byte_t automatic_left;
        automatic byte_t automatic_right;
        automatic byte_t automatic_result;
        automatic_left = 8'h25;
        automatic_right = 8'h34;
        automatic_result = select_left ? automatic_left : automatic_right;
        return automatic_result;
    endfunction

    function automatic byte_t select_automatic_initializer();
        automatic byte_t initialized_value = choose_left ? left_value : right_value;
        return initialized_value;
    endfunction

    function automatic byte_t read_static_initializer();
        static byte_t initialized_value = STATIC_INIT_SELECT ? STATIC_INIT_LEFT : STATIC_INIT_RIGHT;
        return initialized_value;
    endfunction

    function automatic byte_t read_static_snapshot_control();
        // This independent control captures the declaration-uninitialized
        // two-state source's default before the initial process changes it.
        static byte_t initialized_value = static_snapshot_source;
        return initialized_value;
    endfunction

    generate
        if (1'b1) begin : generated_ops
            byte_t generated_conditional_value;
            two_state_t generated_cast_value;
            pair_t generated_pattern_value;
            always_comb begin
                generated_conditional_value = choose_left ? left_value : right_value;
                generated_cast_value = two_state_t'(left_value);
                generated_pattern_value = '{hi: left_value, lo: right_value};
            end
        end
    endgenerate

    initial begin
        choose_left = 1'b1;
        left_value = 8'h25;
        right_value = 8'h34;
        packed_left.hi = 8'h11;
        packed_left.lo = 8'h22;
        packed_right.hi = 8'h33;
        packed_right.lo = 8'h44;
        records_left[0].code = 8'h11;
        records_left[0].data = 8'h22;
        records_left[1].code = 8'h33;
        records_left[1].data = 8'h44;
        records_right[0].code = 8'h11;
        records_right[0].data = 8'h22;
        records_right[1].code = 8'h33;
        records_right[1].data = 8'h45;
        record_source.code = 8'h51;
        record_source.data = 8'h62;
        union_pattern_result = '{union_pattern_left, union_pattern_right};
        constref_lanes[0] = 8'h12;
        constref_lanes[1] = 8'h34;
        compare_left = 8'h00;
        compare_right = 8'h00;
        cast_index = 8'h01;
        pattern_index = 1'b1;
        constref_scalar_control = 8'h5a;
        bus.choose_left = 1'b1;
        bus.left_value = 8'h25;
        bus.right_value = 8'h34;
        color_function_source = COLOR_BLUE;
        union_function_source = union_t'(8'h56);
        // The focal enum and union sources are passed to typed function inputs.
        color_function_result = echo_color(color_function_source);
        union_function_result = echo_union(union_function_source);
        static_event_seen = 1'b0;
        // The observer's event expression uses the qualified static local.
        fork
            drive_static_event_source();
            begin
                @(tb.drive_static_event_source.static_event_source);
                static_event_seen = 1'b1;
            end
        join

        packed_conditional = choose_left ? packed_left : packed_right;
        packed_equality = packed_left == packed_right;
        records_equality = records_left == records_right;
        records_cast = bits32_t'(records_left);
        record_cast = bits16_t'(record_source);

        capture_ops(choose_left ? left_value : right_value,
                    left_value == right_value,
                    two_state_t'(left_value),
                    '{hi: left_value, lo: right_value});
        pattern_from_formals(left_value, right_value);
        // A scalar variable actual is the direct-variable const-ref control.
        capture_selected(constref_scalar_control, constref_scalar_observed);
        capture_selected(constref_lanes[choose_left ? 1 : 0], constref_conditional);
        capture_selected(constref_lanes[compare_left == compare_right], constref_equality);
        capture_selected(constref_lanes[index_bit_t'(cast_index)], constref_cast);
        capture_selected(constref_lanes[index_bit_t'{pattern_index}], constref_pattern);
        static_local_result = select_static_local(choose_left);
        automatic_local_result = select_automatic_local(choose_left);
        automatic_init_result = select_automatic_initializer();
        static_init_first = read_static_initializer();
        static_init_second = read_static_initializer();
        static_snapshot_source = 8'h25;
        static_snapshot_first = read_static_snapshot_control();
        static_snapshot_source = 8'h60;
        static_snapshot_second = read_static_snapshot_control();

        #1;
        if ((packed_conditional.hi !== 8'h11 || packed_conditional.lo !== 8'h22) ||
            packed_equality !== 1'b0)
            $fatal(1, "packed struct operation mismatch");
        if (records_equality !== 1'b0 || records_cast !== 32'h11223344 || record_cast !== 16'h5162)
            $fatal(1, "unpacked record operation mismatch");
        if (union_pattern_result[0].first !== 8'h12 || union_pattern_result[1].second !== 8'h34 ||
            union_pattern_left.second !== 8'h12 || union_pattern_right.first !== 8'h34)
            $fatal(1, "union sources in array pattern mismatch");
        if (color_function_result !== COLOR_BLUE || union_function_result.first !== 8'h56)
            $fatal(1, "typed function argument mismatch");
        if (!static_event_seen)
            $fatal(1, "hierarchical static-local event mismatch");
        if (task_conditional !== 8'h25 || task_equality !== 1'b0 || task_cast !== 8'h25 ||
            task_pattern.hi !== 8'h25 || task_pattern.lo !== 8'h34)
            $fatal(1, "task operation mismatch");
        if (formal_pattern.hi !== 8'h25 || formal_pattern.lo !== 8'h34)
            $fatal(1, "formal assignment-pattern operation mismatch");
        if (constref_scalar_observed !== 8'h5a || constref_conditional !== 8'h34 ||
            constref_equality !== 8'h34 ||
            constref_cast !== 8'h34 || constref_pattern !== 8'h34)
            $fatal(1, "const-ref variable actual mismatch");
        if (static_local_result !== 8'h25 || automatic_local_result !== 8'h25 ||
            automatic_init_result !== 8'h25)
            $fatal(1, "subroutine local operation mismatch");
        if (static_init_first !== 8'h25 || static_init_second !== 8'h25)
            $fatal(1, "static initializer snapshot mismatch");
        if (static_snapshot_first !== 8'h00 || static_snapshot_second !== 8'h00)
            $fatal(1, "static initializer default snapshot mismatch");
        if (CONDITIONAL_CONSTANT !== 8'h22 || $bits(pattern_width_t) != 3)
            $fatal(1, "constant operation mismatch");
        if (generated_ops.generated_conditional_value !== 8'h25 ||
            generated_ops.generated_cast_value !== 8'h25 ||
            generated_ops.generated_pattern_value.hi !== 8'h25 ||
            generated_ops.generated_pattern_value.lo !== 8'h34)
            $fatal(1, "generate operation mismatch");
        if (bus.conditional_value !== 8'h25 || bus.cast_value !== 8'h25 ||
            bus.pattern_value.hi !== 8'h25 || bus.pattern_value.lo !== 8'h34)
            $fatal(1, "interface operation mismatch");

        $display("types=%h,%b,%h,%h,%h task=%h,%b,%h,%h formal=%h constref=%h,%h,%h,%h,%h locals=%h,%h,%h,%h,%h snapshot=%h,%h constant=%h,%0d generate=%h,%h,%h interface=%h,%h,%h",
                 packed_conditional, packed_equality, records_equality, records_cast, record_cast,
                 task_conditional, task_equality, task_cast, task_pattern,
                 formal_pattern, constref_conditional, constref_equality, constref_cast, constref_pattern,
                 constref_scalar_observed,
                 static_local_result, automatic_local_result, automatic_init_result,
                 static_init_first, static_init_second,
                 static_snapshot_first, static_snapshot_second,
                 CONDITIONAL_CONSTANT, $bits(pattern_width_t),
                 generated_ops.generated_conditional_value, generated_ops.generated_cast_value,
                 generated_ops.generated_pattern_value,
                 bus.conditional_value, bus.cast_value, bus.pattern_value);
        $display("union-pattern=%h,%h", union_pattern_result[0].first, union_pattern_result[1].second);
        $display("function-types=%h,%h event=%b", color_function_result, union_function_result.first,
                 static_event_seen);
        $finish(0);
    end
endmodule
