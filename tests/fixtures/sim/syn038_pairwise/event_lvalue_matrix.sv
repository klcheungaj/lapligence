// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/event_lvalue_matrix.sv
// Lvalue forms write persistent objects that explicit event expressions observe.
module tb;
    typedef struct packed { logic [7:0] value; logic [7:0] guard; } record_t;
    typedef struct { logic [7:0] value; logic [7:0] guard; } unpacked_record_t;
    typedef logic [7:0] byte_t;
    typedef byte_t bytes2_t [0:1];
    typedef byte_t bytes3_t [0:2];
    typedef logic [1:0] bits2_t;

    record_t field_slot = 16'h0080;
    record_t guard_control = 16'h1122;
    unpacked_record_t unpacked_field_slot = '{8'h22, 8'h80};
    bytes2_t element_slot = '{0:8'h70, 1:8'h00};
    bytes3_t row_slot = '{0:8'h00, 1:8'h00, 2:8'h73};
    logic [15:0] concat_slot = 16'hA0C0;
    logic [7:0] pattern_slot = 8'h54;

    bit field_armed, guard_armed, unpacked_field_armed;
    bit element_armed, row_armed, concat_armed, pattern_armed;
    bit guard_seen, unpacked_field_seen;
    bit field_seen, element_seen, row_seen, concat_seen, pattern_seen;

    initial begin : wait_field
        field_armed = 1'b1;
        @(field_slot.value);
        field_seen = 1'b1;
    end

    initial begin : wait_guard_control
        guard_armed = 1'b1;
        @(guard_control.value);
        guard_seen = 1'b1;
    end

    initial begin : wait_unpacked_field
        unpacked_field_armed = 1'b1;
        @(unpacked_field_slot.value);
        unpacked_field_seen = 1'b1;
    end

    initial begin : wait_element
        element_armed = 1'b1;
        @(element_slot[1]);
        element_seen = 1'b1;
    end

    initial begin : wait_row
        row_armed = 1'b1;
        @(row_slot[0]);
        row_seen = 1'b1;
    end

    initial begin : wait_concat
        concat_armed = 1'b1;
        @(concat_slot);
        concat_seen = 1'b1;
    end

    initial begin : wait_pattern
        pattern_armed = 1'b1;
        @(pattern_slot);
        pattern_seen = 1'b1;
    end

    initial begin : delayed_writer
        wait (field_armed && guard_armed && unpacked_field_armed && element_armed &&
              row_armed && concat_armed && pattern_armed);
        #1;
        field_slot.value = 8'h31;
        guard_control.guard = 8'h33;
        unpacked_field_slot.value = 8'h35;
        element_slot[1] = 8'h42;
        row_slot[0:1] = '{0:8'h51, 1:8'h52};
        {concat_slot[11:8], concat_slot[3:0]} = 8'h62;
        bits2_t'{pattern_slot[3], pattern_slot[1]} = 2'b10;
    end

    initial begin : check_event_targets
        #2;
        if (!field_seen || guard_seen || !unpacked_field_seen || !element_seen ||
            !row_seen || !concat_seen || !pattern_seen)
            $fatal(1, "event expression did not observe each selected write");
        if (field_slot.value !== 8'h31 || field_slot.guard !== 8'h80 ||
            guard_control.value !== 8'h11 || guard_control.guard !== 8'h33 ||
            unpacked_field_slot.value !== 8'h35 || unpacked_field_slot.guard !== 8'h80 ||
            element_slot[0] !== 8'h70 || element_slot[1] !== 8'h42 ||
            row_slot[0] !== 8'h51 || row_slot[1] !== 8'h52 || row_slot[2] !== 8'h73 ||
            concat_slot !== 16'hA6C2 || pattern_slot !== 8'h5C ||
            concat_slot[15:12] !== 4'hA || concat_slot[7:4] !== 4'hC ||
            pattern_slot[7:4] !== 4'h5 || pattern_slot[2] !== 1'b1 ||
            pattern_slot[0] !== 1'b0)
            $fatal(1, "event lvalue readback or neighboring guard mismatch");
        $display("events=%b%b%b%b%b%b field=%h,%h unpacked=%h,%h element=%h,%h row=%h,%h/%h concat=%h pattern=%h",
                 field_seen, unpacked_field_seen, element_seen, row_seen, concat_seen, pattern_seen,
                 field_slot.value, field_slot.guard, unpacked_field_slot.value,
                 unpacked_field_slot.guard, element_slot[0], element_slot[1],
                 row_slot[0], row_slot[1], row_slot[2], concat_slot, pattern_slot);
        $finish(0);
    end
endmodule
