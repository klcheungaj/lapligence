// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_constant_event_matrix.sv
// Typed constant projections, aggregate initializers, function returns, and events.
typedef struct { logic [2:0] count; logic [4:0] data; } override_record_t;
typedef override_record_t override_records_t [0:1];
typedef struct { logic [7:0] code; logic [7:0] lanes [0:1]; } override_payload_t;

module record_parameter_reader #(
    parameter override_records_t LANES = '{
        0: '{count:3'd1, data:5'h02},
        1: '{count:3'd4, data:5'h12}
    }
) (output logic [2:0] selected_count, output logic [4:0] selected_data);
    initial begin
        selected_count = LANES[1].count;
        selected_data = LANES[1].data;
    end
endmodule

module payload_parameter_reader #(
    parameter override_payload_t PAYLOAD = '{
        code:8'h10,
        lanes:'{0:8'h31, 1:8'h42}
    }
) (output logic [7:0] lane_zero, output logic [7:0] lane_one);
    initial begin
        lane_zero = PAYLOAD.lanes[0];
        lane_one = PAYLOAD.lanes[1];
    end
endmodule

module tb;
    typedef enum logic [2:0] { CODE_ZERO = 3'd0, CODE_EXTENT = 3'd2 } code_t;
    typedef struct packed { logic [2:0] count; logic [4:0] data; } packed_record_t;
    typedef struct { logic [2:0] count; logic [4:0] data; } unpacked_record_t;
    typedef unpacked_record_t record_array_t [0:1];
    typedef union packed { logic [15:0] word; logic [1:0][7:0] bytes; } union_view_t;

    localparam code_t CONSTANT_CODE = CODE_EXTENT;
    localparam packed_record_t CONSTANT_PACKED = '{count:3'd4, data:5'h13};
    localparam record_array_t CONSTANT_ARRAY = '{
        0: '{count:3'd1, data:5'h0a},
        1: '{count:3'd4, data:5'h12}
    };
    localparam unpacked_record_t CONSTANT_RECORD = '{count:3'd3, data:5'h0d};
    localparam union_view_t CONSTANT_UNION = union_view_t'(16'h5aa5);

    function automatic int width_from_auto(input int seed);
        automatic int local_value = seed + 1;
        return local_value;
    endfunction

    function automatic int width_from_static();
        static int local_value;
        local_value = 6;
        return local_value;
    endfunction

    function automatic int width_from_formal(input int seed);
        return seed;
    endfunction

    typedef logic [CONSTANT_CODE:0] enum_extent_t;
    typedef logic [CONSTANT_PACKED.count:0] packed_extent_t;
    typedef logic [CONSTANT_ARRAY[1].count:0] array_record_extent_t;
    typedef logic [CONSTANT_RECORD.count:0] unpacked_record_extent_t;
    typedef logic [width_from_auto(4)-1:0] auto_constant_extent_t;
    typedef logic [width_from_static()-1:0] static_constant_extent_t;
    typedef logic [width_from_formal(7)-1:0] formal_constant_extent_t;

    record_array_t runtime_array = '{
        0: '{count:3'd2, data:5'h05},
        1: '{count:3'd5, data:5'h17}
    };
    unpacked_record_t runtime_record = '{count:3'd4, data:5'h0b};
    packed_record_t packed_event_source = '{count:3'd0, data:5'h01};
    union_view_t union_event_source = union_view_t'(16'h1234);
    union_view_t union_return_value;
    bit events_armed = 1'b0;
    int packed_event_count = 0;
    int union_event_count = 0;
    logic [2:0] override_count_a;
    logic [4:0] override_data_a;
    logic [2:0] override_count_b;
    logic [4:0] override_data_b;
    logic [2:0] default_count;
    logic [4:0] default_data;
    logic [7:0] nested_lane_a_zero;
    logic [7:0] nested_lane_a_one;
    logic [7:0] nested_lane_b_zero;
    logic [7:0] nested_lane_b_one;
    logic [7:0] nested_lane_default_zero;
    logic [7:0] nested_lane_default_one;

    record_parameter_reader #(.LANES('{
        0: '{count:3'd5, data:5'h0c},
        1: '{count:3'd2, data:5'h1a}
    })) override_a (.selected_count(override_count_a), .selected_data(override_data_a));
    record_parameter_reader #(.LANES('{
        0: '{count:3'd6, data:5'h11},
        1: '{count:3'd7, data:5'h03}
    })) override_b (.selected_count(override_count_b), .selected_data(override_data_b));
    record_parameter_reader default_reader (
        .selected_count(default_count), .selected_data(default_data)
    );
    payload_parameter_reader #(.PAYLOAD('{
        code:8'h20,
        lanes:'{0:8'h51, 1:8'h62}
    })) nested_override_a (.lane_zero(nested_lane_a_zero), .lane_one(nested_lane_a_one));
    payload_parameter_reader #(.PAYLOAD('{
        code:8'h30,
        lanes:'{0:8'h73, 1:8'h84}
    })) nested_override_b (.lane_zero(nested_lane_b_zero), .lane_one(nested_lane_b_one));
    payload_parameter_reader nested_default (
        .lane_zero(nested_lane_default_zero), .lane_one(nested_lane_default_one)
    );

    function automatic union_view_t make_union();
        return CONSTANT_UNION;
    endfunction

    always @(packed_event_source) begin
        if (events_armed)
            packed_event_count++;
    end

    always @(union_event_source) begin
        if (events_armed)
            union_event_count++;
    end

    initial begin
        static record_array_t static_array = '{
            0: '{count:3'd3, data:5'h06},
            1: '{count:3'd6, data:5'h1a}
        };
        static unpacked_record_t static_record = '{count:3'd2, data:5'h09};

        if ($bits(enum_extent_t) != 3 || $bits(packed_extent_t) != 5 ||
            $bits(array_record_extent_t) != 5 ||
            $bits(unpacked_record_extent_t) != 4 ||
            $bits(auto_constant_extent_t) != 5 ||
            $bits(static_constant_extent_t) != 6 ||
            $bits(formal_constant_extent_t) != 7)
            $fatal(1, "typed constant elaboration width mismatch");
        if (CONSTANT_ARRAY[1].count !== 3'd4 || CONSTANT_ARRAY[1].data !== 5'h12 ||
            CONSTANT_RECORD.count !== 3'd3 || CONSTANT_RECORD.data !== 5'h0d ||
            runtime_array[1].count !== 3'd5 || runtime_array[1].data !== 5'h17 ||
            runtime_record.count !== 3'd4 || runtime_record.data !== 5'h0b ||
            static_array[1].count !== 3'd6 || static_array[1].data !== 5'h1a ||
            static_record.count !== 3'd2 || static_record.data !== 5'h09)
            $fatal(1, "record initializer readback mismatch");

        union_return_value = make_union();
        if (union_return_value.word !== 16'h5aa5)
            $fatal(1, "packed union function return mismatch");

        #1;
        if (override_count_a !== 3'd2 || override_data_a !== 5'h1a ||
            override_count_b !== 3'd7 || override_data_b !== 5'h03 ||
            default_count !== 3'd4 || default_data !== 5'h12 ||
            nested_lane_a_zero !== 8'h51 || nested_lane_a_one !== 8'h62 ||
            nested_lane_b_zero !== 8'h73 || nested_lane_b_one !== 8'h84 ||
            nested_lane_default_zero !== 8'h31 || nested_lane_default_one !== 8'h42)
            $fatal(1, "parameter aggregate projection lost instance identity");

        events_armed = 1'b1;
        packed_event_source = '{count:3'd1, data:5'h1b};
        union_event_source = union_view_t'(16'habcd);
        #1;
        if (packed_event_count != 1 || union_event_count != 1)
            $fatal(1, "typed aggregate event source mismatch");

        $display("widths=%0d,%0d,%0d,%0d constfunc=%0d,%0d,%0d const=%0d/%h,%0d/%h runtime=%0d/%h,%0d/%h static=%0d/%h,%0d/%h return=%h overrides=%0d/%h,%0d/%h,%0d/%h nested=%h,%h,%h,%h,%h,%h events=%0d,%0d",
            $bits(enum_extent_t), $bits(packed_extent_t), $bits(array_record_extent_t),
            $bits(unpacked_record_extent_t), $bits(auto_constant_extent_t),
            $bits(static_constant_extent_t), $bits(formal_constant_extent_t),
            CONSTANT_ARRAY[1].count, CONSTANT_ARRAY[1].data,
            CONSTANT_RECORD.count, CONSTANT_RECORD.data, runtime_array[1].count,
            runtime_array[1].data, runtime_record.count, runtime_record.data,
            static_array[1].count, static_array[1].data, static_record.count,
            static_record.data, union_return_value.word,
            override_count_a, override_data_a, override_count_b, override_data_b,
            default_count, default_data, nested_lane_a_zero, nested_lane_a_one,
            nested_lane_b_zero, nested_lane_b_one, nested_lane_default_zero,
            nested_lane_default_one, packed_event_count, union_event_count);
        $finish(0);
    end
endmodule
