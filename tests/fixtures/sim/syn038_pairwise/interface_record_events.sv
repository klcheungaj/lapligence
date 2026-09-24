// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_record_events.sv
// IEEE 1800-2009 §§7.2, 7.12.3, 9.4, 11.4, and 25.2: separate interface
// event expressions observe a fixed-record reduction and a record-field predicate.
interface record_event_if;
    typedef struct {
        logic [7:0] key;
        logic [7:0] payload;
    } record_t;

    typedef struct {
        logic [7:0] value;
    } entry_t;
    typedef entry_t record_array_t [0:1];

    localparam logic [7:0] known_byte = 8'hA5;
    record_t store;
    record_array_t records;
    bit reduction_armed;
    bit field_armed;
    int reduction_events = 0;
    int field_events = 0;

    // Focal vector: fixed_array_record, direct_projection, event_expression,
    // field, interface_member, none, interface, local,
    // fixed_array_reduction, none, none, procedural_blocking, always.
    always @(records.sum() with (int'(item.value))) begin
        if (reduction_armed)
            reduction_events = reduction_events + 1;
    end

    // The equality result is one bit; this focal value is store's record field.
    // Focal vector: unpacked_record, equality_inside, event_expression, field,
    // interface_member, none, interface, local, none, none, none,
    // procedural_blocking, always.
    always @(store.key == known_byte) begin
        if (field_armed)
            field_events = field_events + 1;
    end
endinterface

module tb;
    record_event_if bus();

    initial begin
        bus.reduction_armed = 1'b0;
        bus.field_armed = 1'b0;
        bus.records[0].value = 8'd1;
        bus.records[1].value = 8'd2;
        bus.store.key = 8'h11;
        bus.store.payload = 8'h5A;

        #1;
        if (bus.reduction_events != 0 || bus.field_events != 0)
            $fatal(1, "initial interface values counted as source events");
        bus.reduction_armed = 1'b1;
        bus.field_armed = 1'b1;

        #1;
        bus.records[0].value = 8'd4;
        bus.store.key = 8'hA5;

        #1;
        if (bus.reduction_events != 1)
            $fatal(1, "interface record reduction event count mismatch");
        if (bus.field_events != 1)
            $fatal(1, "interface record field event count mismatch");
        $display("events=%0d,%0d", bus.reduction_events, bus.field_events);
        $finish(0);
    end
endmodule
