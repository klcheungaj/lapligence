// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_context_remainders.sv
// IEEE 1800-2009 §§6.19, 6.24, 7.2, 7.4, 11.4, and 11.8: remaining typed
// operation, consumer, initializer, and selected-write contexts.
module parameter_override_reader #(
    parameter logic [7:0] lanes [0:1] = '{8'h21, 8'h43}
) (output logic [7:0] selected);
    int index;
    initial begin
        index = 1;
        selected = lanes[index];
    end
endmodule

module tb;
    typedef enum logic [1:0] { SLEEP = 2'b00, AWAKE = 2'b01 } phase_t;
    typedef phase_t phase_lanes_t [0:1];
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    typedef union packed { logic [15:0] word; pair_t halves; } word_view_t;
    typedef logic [7:0] byte_t;
    typedef byte_t lanes_t [0:1];
    typedef lanes_t lane_grid_t [0:1];
    typedef struct { byte_t code; byte_t data; } record_t;
    typedef record_t records_t [0:1];
    typedef records_t record_rows_t [0:1];
    typedef struct { byte_t code; byte_t lanes [0:1]; } payload_t;

    function automatic word_view_t make_union_value();
        return word_view_t'(16'hC0DE);
    endfunction

    localparam phase_t CONST_PHASE = AWAKE;
    localparam pair_t CONST_PAIR = '{hi:8'h12, lo:8'h34};
    localparam lanes_t CONST_LANES = '{0:8'h21, 1:8'h43};
    localparam lane_grid_t CONST_GRID = '{
        0: '{0:8'h11, 1:8'h22},
        1: '{0:8'h33, 1:8'h44}
    };
    localparam records_t CONST_RECORDS = '{
        0: '{code:8'h11, data:8'h22},
        1: '{code:8'h33, data:8'h44}
    };
    localparam payload_t CONST_PAYLOAD = '{
        code:8'h55,
        lanes:'{0:8'h66, 1:8'h77}
    };

    // Each parameter-sized type consumes one focal value during constant
    // elaboration; the member and element projections retain the outer type.
    typedef logic [(int'(CONST_PHASE) % 3):0] phase_width_t;
    typedef logic [(int'(CONST_PAIR.hi) % 7):0] pair_width_t;
    typedef logic [(int'(CONST_LANES[0]) % 7):0] lane_width_t;
    typedef logic [(int'(CONST_RECORDS[0].code) % 7):0] record_array_width_t;
    typedef logic [(int'(CONST_PAYLOAD.code) % 7):0] payload_width_t;
    typedef logic [(int'(CONST_PAYLOAD.lanes[0]) % 7):0] payload_lane0_width_t;
    typedef logic [(int'(CONST_PAYLOAD.lanes[1]) % 7):0] payload_lane1_width_t;

    typedef logic [15:0] bits16_t;
    typedef logic [23:0] bits24_t;
    typedef logic [31:0] bits32_t;

    phase_lanes_t phase_source = '{SLEEP, AWAKE};
    phase_t phase_first;
    phase_t phase_second;

    pair_t pair_source = '{hi:8'h12, lo:8'h34};
    pair_t pair_alternate = '{hi:8'h56, lo:8'h78};
    pair_t pair_selected;
    logic select_pair = 1'b1;
    logic pair_equal;
    logic [7:0] override_selected;
    parameter_override_reader #(.lanes('{8'h51, 8'h62})) override_reader (
        .selected(override_selected)
    );

    word_view_t union_pattern = word_view_t'(16'h5A6B);
    word_view_t union_event_source = word_view_t'(16'h1234);
    word_view_t union_event_goal = word_view_t'(16'h1235);
    word_view_t union_return_value;
    int union_event_count = 0;
    bit events_armed = 1'b0;

    pair_t event_pair_source = '{hi:8'h11, lo:8'h22};
    pair_t event_pair_goal = '{hi:8'h33, lo:8'h44};
    int pair_event_count = 0;

    lanes_t runtime_lanes = '{0:8'h21, 1:8'h43};
    int parameter_index;
    byte_t parameter_selected;
    records_t runtime_records = '{
        0: '{code:8'h11, data:8'h22},
        1: '{code:8'h33, data:8'h44}
    };
    payload_t runtime_payload = '{
        code:8'h55,
        lanes:'{0:8'h66, 1:8'h77}
    };

    bits16_t lanes_cast;
    bits16_t grid_row_cast;
    bits32_t records_cast;
    bits24_t payload_cast;
    logic lanes_inside;
    logic records_equal;
    logic [7:0] override_selected_second;
    logic [7:0] default_selected;

    parameter_override_reader #(.lanes('{8'h71, 8'h82})) override_reader_second (
        .selected(override_selected_second)
    );
    parameter_override_reader default_reader (.selected(default_selected));

    lanes_t integral_target = '{0:8'hA0, 1:8'hA1};
    lanes_t integral_pattern_source = '{0:8'h30, 1:8'h31};
    record_rows_t record_rows_target;
    records_t record_pattern_source = '{
        0: '{code:8'h60, data:8'h62},
        1: '{code:8'h64, data:8'h66}
    };
    payload_t payload_target = '{code:8'hD0, lanes:'{0:8'hD1, 1:8'hD2}};
    payload_t payload_pattern_source = '{code:8'h90, lanes:'{0:8'h91, 1:8'h92}};

    always @(event_pair_source == event_pair_goal) begin
        if (events_armed)
            pair_event_count = pair_event_count + 1;
    end

    always @(union_event_source == union_event_goal) begin
        if (events_armed)
            union_event_count = union_event_count + 1;
    end

    initial begin : check
        static records_t static_records = '{
            0: '{code:8'h61, data:8'h62},
            1: '{code:8'h63, data:8'h64}
        };

        pair_selected = select_pair ? pair_source : pair_alternate;
        pair_equal = pair_source == pair_alternate;
        records_equal = runtime_records == CONST_RECORDS;
        union_return_value = make_union_value();
        lanes_cast = bits16_t'(runtime_lanes);
        grid_row_cast = bits16_t'(CONST_GRID[1]);
        records_cast = bits32_t'(runtime_records);
        payload_cast = bits24_t'(runtime_payload);
        lanes_inside = 8'h43 inside {runtime_lanes};
        phase_lanes_t'{phase_first, phase_second} = phase_source;

        // Runtime selection from a constant fixed-array parameter follows
        // the same declaration-order bounds as an ordinary fixed array.
        parameter_index = 0;
        parameter_selected = CONST_LANES[parameter_index];
        if (parameter_selected !== 8'h21)
            $fatal(1, "typed array parameter runtime index zero mismatch");
        parameter_index = 1;
        parameter_selected = CONST_LANES[parameter_index];
        if (parameter_selected !== 8'h43)
            $fatal(1, "typed array parameter runtime index one mismatch");
        parameter_index = 2;
        parameter_selected = CONST_LANES[parameter_index];
        if (parameter_selected !== 8'hxx)
            $fatal(1, "typed array parameter out-of-range index mismatch");

        // Array-valued row slices, concatenations, and positional patterns.
        integral_target[0:1] = '{0:8'h10, 1:8'h11};
        lanes_t'{integral_target[0], integral_target[1]} = integral_pattern_source;
        {integral_target[0], integral_target[1]} = {8'h20, 8'h21};

        // Array-of-record row slice, followed by projected-leaf concat and
        // positional-pattern writes to the same outer fixed-array slot.
        record_rows_target[0:1] = '{
            0: '{0: '{code:8'h40, data:8'h41}, 1: '{code:8'h42, data:8'h43}},
            1: '{0: '{code:8'h44, data:8'h45}, 1: '{code:8'h46, data:8'h47}}
        };
        record_rows_target[0][0:1] = '{
            0: '{code:8'h48, data:8'h49},
            1: '{code:8'h4A, data:8'h4B}
        };
        if (record_rows_target[0][0].code !== 8'h48 ||
            record_rows_target[0][0].data !== 8'h49 ||
            record_rows_target[0][1].code !== 8'h4A ||
            record_rows_target[0][1].data !== 8'h4B)
            $fatal(1, "typed fixed-record-array row-slice target mismatch");
        {record_rows_target[0][0].code, record_rows_target[0][1].data} =
            {8'h50, 8'h51};
        records_t'{record_rows_target[1][0], record_rows_target[1][1]} =
            record_pattern_source;
        if (record_rows_target[1][0].code !== 8'h60 ||
            record_rows_target[1][0].data !== 8'h62 ||
            record_rows_target[1][1].code !== 8'h64 ||
            record_rows_target[1][1].data !== 8'h66)
            $fatal(1, "typed fixed-record-array positional-pattern target mismatch");

        // Unpacked-record member-array row slice and leaf-address forms.
        payload_target.lanes[0:1] = '{0:8'h70, 1:8'h71};
        if (payload_target.code !== 8'hD0 || payload_target.lanes[0] !== 8'h70 ||
            payload_target.lanes[1] !== 8'h71)
            $fatal(1, "typed unpacked-record row-slice target mismatch");
        payload_t'{payload_target.code, payload_target.lanes} = payload_pattern_source;
        if (payload_target.code !== 8'h90 || payload_target.lanes[0] !== 8'h91 ||
            payload_target.lanes[1] !== 8'h92)
            $fatal(1, "typed unpacked-record positional-pattern target mismatch");
        {payload_target.code, payload_target.lanes[0]} = {8'h80, 8'h81};

        if ($bits(phase_width_t) != 2 || $bits(pair_width_t) != 5 ||
            $bits(lane_width_t) != 6 || $bits(record_array_width_t) != 4 ||
            $bits(payload_width_t) != 2 || $bits(payload_lane0_width_t) != 5 ||
            $bits(payload_lane1_width_t) != 1)
            $fatal(1, "typed constant elaboration mismatch");
        if (CONST_PAYLOAD.code !== 8'h55 || CONST_PAYLOAD.lanes[0] !== 8'h66 ||
            CONST_PAYLOAD.lanes[1] !== 8'h77)
            $fatal(1, "typed constant unpacked-record member mismatch");
        if (grid_row_cast !== 16'h3344)
            $fatal(1, "typed constant array row read mismatch");
        // Keep a procedural fixed-array parameter read as well as the
        // elaboration-time uses above; parameter arrays have no runtime cell.
        if (CONST_LANES[0] !== 8'h21)
            $fatal(1, "typed array parameter read mismatch");
        if (union_pattern.word !== 16'h5A6B ||
            runtime_lanes[0] !== 8'h21 || runtime_lanes[1] !== 8'h43 ||
            runtime_records[0].code !== 8'h11 || runtime_records[1].data !== 8'h44 ||
            runtime_payload.code !== 8'h55 || runtime_payload.lanes[1] !== 8'h77 ||
            static_records[0].code !== 8'h61 || static_records[0].data !== 8'h62 ||
            static_records[1].code !== 8'h63 || static_records[1].data !== 8'h64)
            $fatal(1, "typed runtime or static initializer mismatch");
        if (pair_selected !== CONST_PAIR || pair_equal !== 1'b0 || records_equal !== 1'b1 ||
            lanes_cast !== 16'h2143 || records_cast !== 32'h11223344 ||
            payload_cast !== 24'h556677 || lanes_inside !== 1'b1 ||
            phase_first !== SLEEP || phase_second !== AWAKE ||
            union_return_value.word !== 16'hC0DE)
            $fatal(1, "typed source operation mismatch");
        if (integral_target[0] !== 8'h20 || integral_target[1] !== 8'h21 ||
            record_rows_target[0][0].code !== 8'h50 ||
            record_rows_target[0][0].data !== 8'h49 ||
            record_rows_target[0][1].code !== 8'h4A ||
            record_rows_target[0][1].data !== 8'h51 ||
            record_rows_target[1][0].code !== 8'h60 ||
            record_rows_target[1][0].data !== 8'h62 ||
            record_rows_target[1][1].code !== 8'h64 ||
            record_rows_target[1][1].data !== 8'h66 ||
            payload_target.code !== 8'h80 || payload_target.lanes[0] !== 8'h81 ||
            payload_target.lanes[1] !== 8'h92)
            $fatal(1, "typed selected target mismatch");

        events_armed = 1'b1;
        event_pair_source = event_pair_goal;
        union_event_source = union_event_goal;
        #1;
        if (pair_event_count != 1 || union_event_count != 1)
            $fatal(1, "typed equality event mismatch");
        if (override_selected !== 8'h62 || override_selected_second !== 8'h82 ||
            default_selected !== 8'h43)
            $fatal(1, "typed overridden array parameter read mismatch");

        $display("const=%0d,%0d,%0d,%0d,%0d op=%h,%b,%h,%h,%h,%b init=%h,%h,%h,%h,%h,%h target=%h,%h,%h,%h events=%0d,%0d row=%h override=%h,%h,%h extra=%b,%0d,%0d,%h",
                 $bits(phase_width_t), $bits(pair_width_t), $bits(lane_width_t),
                 $bits(record_array_width_t), $bits(payload_width_t),
                 pair_selected, pair_equal, lanes_cast, records_cast, payload_cast,
                 lanes_inside, union_pattern.word, runtime_lanes[0], runtime_records[0].code,
                 runtime_payload.code, runtime_payload.lanes[1], static_records[1].data,
                 integral_target[0], record_rows_target[1][0].code,
                 payload_target.lanes[0], payload_target.lanes[1],
                 pair_event_count, union_event_count, grid_row_cast, override_selected,
                 override_selected_second, default_selected, records_equal, phase_first,
                 phase_second, union_return_value.word);
        $finish(0);
    end
endmodule
