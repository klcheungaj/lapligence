// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/lvalue_storage_formal_matrix.sv
// Selected SYN-038 lvalue shapes across storage, formal, call, and function contexts.
module output_source(output logic [1:0] value);
    assign value = 2'b10;
endmodule

module ref_slice_source(ref logic [3:0] value);
    initial value = 4'ha;
endmodule

module tb;
    typedef logic [7:0] byte_t;
    typedef byte_t pair_t [0:1];
    typedef struct { byte_t code; byte_t data; } record_t;
    typedef union packed { logic [15:0] word; logic [15:0] code; } u_t;
    typedef u_t us_t [0:1];

    us_t union_source = '{u_t'(16'h1234), u_t'(16'h5678)};

    task automatic write_output(output logic [1:0] value);
        value = 2'b10;
    endtask

    task automatic bump_pattern(inout logic [1:0] value);
        value = value + 1'b1;
    endtask

    task automatic bump_concatenation(inout logic [1:0] value);
        value = value + 1'b1;
    endtask

    task automatic write_formal_pattern(output pair_t value);
        pair_t'{value[0], value[1]} = '{0:8'h11, 1:8'h12};
    endtask

    task automatic write_task_local(output byte_t result);
        pair_t task_local;
        pair_t'{task_local[0], task_local[1]} = '{0:8'h91, 1:8'h92};
        result = task_local[1];
    endtask

    task automatic write_byte_ref(ref byte_t value);
        value = 8'h7a;
    endtask

    function automatic logic [1:0] write_function_output(output logic [1:0] value);
        value = 2'b10;
        write_function_output = 2'b01;
    endfunction

    function automatic logic [1:0] make_return_pattern;
        '{make_return_pattern[1], make_return_pattern[0]} = 2'b10;
    endfunction

    function automatic record_t make_record;
        make_record.code = 8'h61;
        make_record.data = 8'h62;
    endfunction

    function automatic pair_t make_row_slice_return;
        make_row_slice_return[0:1] = '{0:8'h31, 1:8'h32};
    endfunction

    function automatic logic [15:0] make_concat_return;
        {make_concat_return[15:8], make_concat_return[7:0]} = 16'h4142;
    endfunction

    int low_index_calls = 0;
    int high_index_calls = 0;
    function automatic int low_index;
        low_index_calls = low_index_calls + 1;
        low_index = 0;
    endfunction

    function automatic int high_index;
        high_index_calls = high_index_calls + 1;
        high_index = 1;
    endfunction

    logic port_hi, port_lo;
    output_source u_output(.value('{port_hi, port_lo}));
    logic [7:0] packed_storage = 8'h00;
    ref_slice_source u_ref_slice(.value(packed_storage[7:4]));

    logic task_hi, task_lo;
    logic function_hi, function_lo;
    logic [1:0] function_result;
    logic index_targets [0:1];
    logic [1:0] direct_output;
    logic concat_output_hi, concat_output_lo;
    logic [1:0] pattern_inout = 2'b10;
    logic concat_inout_hi = 1'b1, concat_inout_lo = 1'b0;
    logic [15:0] concat_function_return;
    record_t function_record;
    pair_t row_function_target;
    u_t first_union, second_union;
    pair_t formal_target;
    logic [1:0] return_pattern;
    byte_t task_local_result;
    record_t ref_record = '{code:8'h00, data:8'h00};

    initial begin : check
        static pair_t static_local;
        automatic pair_t automatic_local;

        // CO port_actual × LV positional_pattern.
        // CO call_argument × LV positional_pattern; the dynamic selectors are
        // each evaluated once and the output-only targets are not read first.
        write_output('{task_hi, task_lo});
        write_output('{index_targets[low_index()], index_targets[high_index()]});
        write_output(direct_output);
        write_output({concat_output_hi, concat_output_lo});
        function_result = write_function_output('{function_hi, function_lo});

        // LV positional_pattern × FM inout, plus the concatenation/inout control.
        bump_pattern('{pattern_inout[1], pattern_inout[0]});
        bump_concatenation({concat_inout_hi, concat_inout_lo});

        // LV positional_pattern × SL static_local/automatic_local/formal;
        // the function body separately exercises a positional return-slot write.
        pair_t'{static_local[0], static_local[1]} = '{0:8'h21, 1:8'h22};
        pair_t'{automatic_local[0], automatic_local[1]} = '{0:8'h23, 1:8'h24};
        write_formal_pattern(formal_target);
        write_task_local(task_local_result);
        return_pattern = make_return_pattern();

        // TY untagged_packed_union × LV positional_pattern: each typed
        // component target receives one union value from the fixed-array source.
        us_t'{first_union, second_union} = union_source;

        // These functions write the exact returned focal slot through the
        // field, row-slice, concatenation, and positional-pattern addresses.
        function_record = make_record();
        row_function_target = make_row_slice_return();
        concat_function_return = make_concat_return();

        // LV field × FM ref and LV row_slice × FM ref through a module ref port.
        write_byte_ref(ref_record.code);

        #1;
        if ({port_hi, port_lo} !== 2'b10 || {task_hi, task_lo} !== 2'b10 ||
            {function_hi, function_lo} !== 2'b10 || function_result !== 2'b01 ||
            low_index_calls != 1 || high_index_calls != 1 ||
            index_targets[0] !== 1'b1 || index_targets[1] !== 1'b0 ||
            direct_output !== 2'b10 || {concat_output_hi, concat_output_lo} !== 2'b10 ||
            pattern_inout !== 2'b11 || {concat_inout_hi, concat_inout_lo} !== 2'b11 ||
            function_record.code !== 8'h61 || function_record.data !== 8'h62 ||
            row_function_target[0] !== 8'h31 || row_function_target[1] !== 8'h32 ||
            concat_function_return !== 16'h4142 ||
            formal_target[0] !== 8'h11 || formal_target[1] !== 8'h12 ||
            static_local[0] !== 8'h21 || static_local[1] !== 8'h22 ||
            automatic_local[0] !== 8'h23 || automatic_local[1] !== 8'h24 ||
            return_pattern !== 2'b10 || task_local_result !== 8'h92 ||
            first_union.word !== 16'h1234 || second_union.word !== 16'h5678 ||
            packed_storage !== 8'ha0 || ref_record.code !== 8'h7a)
            $fatal(1, "lvalue storage/formal matrix mismatch");

        $display("union=%h,%h port=%b%b task=%b%b function=%b%b:%b dynamic=%b%b/%0d,%0d direct=%b concat=%b%b inout=%b concat_inout=%b%b concat_fn=%h field=%h,%h row=%h,%h formal=%h,%h static=%h,%h automatic=%h,%h return=%b task_local=%h ref=%h ref_field=%h",
                 first_union.word, second_union.word,
                 port_hi, port_lo, task_hi, task_lo, function_hi, function_lo,
                 function_result, index_targets[0], index_targets[1],
                 low_index_calls, high_index_calls,
                 direct_output, concat_output_hi, concat_output_lo, pattern_inout,
                 concat_inout_hi, concat_inout_lo, concat_function_return, function_record.code,
                 function_record.data, row_function_target[0], row_function_target[1],
                 formal_target[0], formal_target[1], static_local[0], static_local[1], automatic_local[0],
                 automatic_local[1], return_pattern, task_local_result, packed_storage,
                 ref_record.code);
        $finish(0);
    end
endmodule
