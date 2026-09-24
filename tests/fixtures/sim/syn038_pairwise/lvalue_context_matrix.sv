// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/lvalue_context_matrix.sv
// IEEE 1800-2009 §§9.2, 10.5, 10.9, 13, and 23.3: process, formal-actual,
// interface-member, and child-port lvalue paths with distinct storage oracles.
module concat_source(output logic [7:0] value);
    initial value = 8'h9a;
endmodule

interface target_if;
    logic [7:0] elements [0:1];
    logic [7:0] rows [0:1];
    logic [7:0] concat_data;
    logic [7:0] pattern_data = 8'h80;
    logic [7:0] seeded = 8'h12;
endinterface

module tb;
    typedef struct packed { logic [7:0] field; } record_t;

    logic clk = 1'b0;
    logic comb_enable = 1'b1;
    logic latch_enable = 1'b0;
    logic [7:0] runtime_seed = 8'h23;
    localparam logic [7:0] constant_seed = 8'h34;

    record_t always_record;
    logic [7:0] always_elements [0:1];
    logic [7:0] always_rows [0:1];
    logic [7:0] always_concat;
    logic [7:0] always_pattern = 8'h80;

    logic [7:0] comb_concat;
    logic [7:0] comb_pattern = 8'h01;
    logic [7:0] latch_concat;
    logic [7:0] latch_pattern = 8'h00;
    logic [7:0] ff_rows [0:1];
    logic [7:0] ff_pattern = 8'h80;


    record_t call_record;
    logic [7:0] call_elements [0:1];
    logic [7:0] call_rows [0:1];
    logic [7:0] call_task_rows [0:1];
    logic [7:0] call_concat_fn;
    logic [7:0] call_concat_task;

    logic [7:0] child_concat;
    target_if bus();
    concat_source child(.value({child_concat[7:4], child_concat[3:0]}));

    function automatic logic [7:0] read_interface_seed();
        return bus.seeded;
    endfunction

    function automatic logic [7:0] read_static_function_seed();
        static logic [7:0] stored = 8'h67;
        return stored;
    endfunction

    function automatic logic [7:0] make_initializer_value();
        return 8'h78;
    endfunction

    function automatic logic [7:0] write_field(
        input logic [7:0] source, output logic [7:0] target);
        target = source;
        return 8'h00;
    endfunction

    function automatic logic [7:0] write_element(
        input logic [7:0] source, output logic [7:0] target);
        target = source;
        return 8'h00;
    endfunction

    function automatic logic [7:0] write_row_function(
        input logic [3:0] source, output logic [3:0] target);
        target = source;
        return 8'h00;
    endfunction

    task automatic write_row_task(input logic [3:0] source, output logic [3:0] target);
        target = source;
    endtask

    function automatic logic [7:0] write_concat_function(
        input logic [7:0] source, output logic [7:0] target);
        target = source;
        return 8'h00;
    endfunction

    task automatic write_concat_task(input logic [7:0] source, output logic [7:0] target);
        target = source;
    endtask

    always @(posedge clk) begin
        always_record.field = 8'h11;
        always_elements[1] = 8'h22;
        always_rows[0][7:4] = 4'h3;
        {always_concat[7:4], always_concat[3:0]} = 8'h45;
        '{always_pattern[7], always_pattern[0]} = 2'b01;

    end

    always_comb begin
        if (comb_enable) begin
            {comb_concat[7:4], comb_concat[3:0]} = 8'h89;
            '{comb_pattern[7], comb_pattern[0]} = 2'b10;
        end else begin
            {comb_concat[7:4], comb_concat[3:0]} = 8'h00;
            '{comb_pattern[7], comb_pattern[0]} = 2'b00;
        end
    end

    always_latch if (latch_enable) begin
        {latch_concat[7:4], latch_concat[3:0]} = 8'hcd;
        '{latch_pattern[7], latch_pattern[0]} = 2'b11;
    end

    always_ff @(posedge clk) begin
        ff_rows[0][7:4] <= 4'h5;
        '{ff_pattern[7], ff_pattern[0]} <= 2'b01;
    end

    initial begin
        logic [7:0] call_field_result;
        logic [7:0] call_element_result;
        logic [7:0] call_row_fn_result;
        logic [7:0] call_concat_fn_result;
        static logic [7:0] static_seed = 8'h45;
        automatic logic [7:0] automatic_seed = 8'h56;
        automatic logic [7:0] from_static_seed = static_seed;
        automatic logic [7:0] from_interface_seed = bus.seeded;
        automatic logic [7:0] from_return_slot = make_initializer_value();
        logic [7:0] static_function_result;
        logic [7:0] read_interface_seed_value;
        #1;
        static_function_result = read_static_function_seed();
        read_interface_seed_value = read_interface_seed();
        clk = 1'b1;
        latch_enable = 1'b1;

        bus.elements[1] = 8'h31;
        bus.rows[0][7:4] = 4'h4;
        {bus.concat_data[7:4], bus.concat_data[3:0]} = 8'h56;
        '{bus.pattern_data[7], bus.pattern_data[0]} = 2'b01;

        call_field_result = write_field(8'hc1, call_record.field);
        call_element_result = write_element(8'hd2, call_elements[1]);
        call_row_fn_result = write_row_function(4'h6, call_rows[0][7:4]);
        write_row_task(4'h7, call_task_rows[0][7:4]);
        call_concat_fn_result = write_concat_function(8'he3, {call_concat_fn[7:4], call_concat_fn[3:0]});
        write_concat_task(8'hf4, {call_concat_task[7:4], call_concat_task[3:0]});

        #1;
        if (always_record.field !== 8'h11 || always_elements[1] !== 8'h22 ||
            always_rows[0][7:4] !== 4'h3 ||
            always_concat !== 8'h45 || always_pattern !== 8'h01)
            $fatal(1, "always lvalue forms mismatch");
        if (comb_concat !== 8'h89 || comb_pattern !== 8'h80)
            $fatal(1, "always_comb lvalue forms mismatch");
        if (latch_concat !== 8'hcd || latch_pattern !== 8'h81)
            $fatal(1, "always_latch lvalue forms mismatch");
        if (ff_rows[0][7:4] !== 4'h5 || ff_pattern !== 8'h01)
            $fatal(1, "always_ff lvalue forms mismatch");
        if (bus.elements[1] !== 8'h31 || bus.rows[0][7:4] !== 4'h4 ||
            bus.concat_data !== 8'h56 || bus.pattern_data !== 8'h01)
            $fatal(1, "interface-member lvalue forms mismatch");
        if (call_field_result !== 8'h00 || call_element_result !== 8'h00 ||
            call_row_fn_result !== 8'h00 || call_concat_fn_result !== 8'h00 ||
            call_record.field !== 8'hc1 || call_elements[1] !== 8'hd2 ||
            call_rows[0][7:4] !== 4'h6 || call_task_rows[0][7:4] !== 4'h7 ||
            call_concat_fn !== 8'he3 || call_concat_task !== 8'hf4)
            $fatal(1, "function/task lvalue actual mismatch");
        if (runtime_seed !== 8'h23 || constant_seed !== 8'h34 ||
            static_seed !== 8'h45 || automatic_seed !== 8'h56 || bus.seeded !== 8'h12 ||
            from_static_seed !== 8'h45 || from_interface_seed !== 8'h12 ||
            from_return_slot !== 8'h78 || static_function_result !== 8'h67 ||
            read_interface_seed_value !== 8'h12)
            $fatal(1, "declaration initializer mismatch");
        if (child_concat !== 8'h9a)
            $fatal(1, "child output concatenation actual mismatch");

        $display("proc=%h,%h,%h,%h,%h comb=%h,%h latch=%h,%h ff=%h,%h if=%h,%h,%h,%h call=%h,%h,%h,%h,%h,%h child=%h init=%h,%h,%h,%h,%h extra=%h,%h,%h,%h,%h",
            always_record.field, always_elements[1], always_rows[0][7:4],
            always_concat, always_pattern, comb_concat, comb_pattern, latch_concat, latch_pattern,
            ff_rows[0][7:4], ff_pattern,
            bus.elements[1], bus.rows[0][7:4], bus.concat_data, bus.pattern_data,
            call_record.field, call_elements[1], call_rows[0][7:4], call_task_rows[0][7:4],
            call_concat_fn, call_concat_task, child_concat, runtime_seed, constant_seed,
            static_seed, automatic_seed, bus.seeded,
            from_static_seed, from_interface_seed, from_return_slot,
            static_function_result, read_interface_seed_value);
        $finish(0);
    end
endmodule
