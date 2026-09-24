// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/hierarchical_value_matrix.sv
// Hierarchical reads cover typed values, operation contexts, a port actual,
// function return, and an explicit child enum event. Selected writes check
// field, element, slice, concatenation, and positional-pattern projections.
package data_types;
    typedef enum logic [7:0] {IDLE = 8'h00, ACTIVE = 8'h01} state_t;
    typedef struct packed {
        logic [7:0] hi;
        logic [7:0] lo;
    } pair_t;
    typedef struct {
        logic [7:0] key;
        logic [7:0] payload;
    } record_t;
    typedef record_t records_t [0:1];
    typedef logic [7:0] bytes_t [0:1];
    typedef logic [7:0] byte_t;
endpackage

module child;
    import data_types::*;

    state_t state = ACTIVE;
    pair_t pair = '{hi: 8'h12, lo: 8'h34};
    record_t record = '{key: 8'h21, payload: 8'h43};
    records_t records = '{
        0: '{key: 8'h11, payload: 8'h22},
        1: '{key: 8'h33, payload: 8'h44}
    };
    bytes_t bytes = '{8'h56, 8'h78};
    logic [7:0] bits = 8'h00;
endmodule

module receiver(input logic [7:0] value, output logic [7:0] observed);
    assign observed = value;
endmodule

module tb;
    import data_types::*;

    child u();
    logic [7:0] port_readback;
    receiver r(.value(u.pair.hi), .observed(port_readback));
    logic select = 1'b1;
    logic [7:0] conditional_read;
    logic equality_read;
    logic [7:0] cast_read;
    logic [7:0] pattern_read;
    logic [7:0] record_read;
    logic [7:0] records_read;
    logic [7:0] bytes_read;
    int event_count;
    logic armed;

    function automatic logic [7:0] read_pair;
        return u.pair.hi;
    endfunction

    always @(u.state)
        if (armed)
            event_count++;

    initial begin
        event_count = 0;
        armed = 0;
        #1;

        conditional_read = select ? u.state : IDLE;
        equality_read = u.state == ACTIVE;
        cast_read = byte_t'(u.pair.hi);
        pattern_read = '{
            u.bytes[0][7], u.bytes[0][6], u.bytes[0][5], u.bytes[0][4],
            u.bytes[0][3], u.bytes[0][2], u.bytes[0][1], u.bytes[0][0]
        };
        record_read = u.record.key;
        records_read = u.records[1].key;
        bytes_read = u.bytes[0];
        if (conditional_read != 1 || equality_read != 1 || cast_read != 8'h12 ||
            pattern_read != 8'h56 || record_read != 8'h21 || records_read != 8'h33 ||
            bytes_read != 8'h56 || port_readback != 8'h12 || read_pair() != 8'h12)
            $fatal(1, "hier reads");

        armed = 1;
        u.state = IDLE;
        #1;
        if (event_count != 1)
            $fatal(1, "hier event");

        u.pair.hi = 8'ha1;
        if (u.pair.hi !== 8'ha1)
            $fatal(1, "field write");
        u.bytes[0] = 8'hb2;
        if (u.bytes[0] !== 8'hb2)
            $fatal(1, "element write");
        u.bits[7:4] = 4'hc;
        if (u.bits !== 8'hc0)
            $fatal(1, "slice write");
        {u.bits[7:4], u.bits[3:0]} = 8'hd2;
        if (u.bits !== 8'hd2)
            $fatal(1, "concat write");
        '{u.bits[7], u.bits[0]} = 2'b01;
        if (u.bits !== 8'h53)
            $fatal(1, "pattern write");

        if (u.pair.hi != 8'ha1 || u.bytes[0] != 8'hb2 || u.bits != 8'h53)
            $fatal(1, "hier writes");
        $display("hier=%h,%h,%h,%h,%h,%h,%h,%h,%h event=%0d",
                 conditional_read, equality_read, cast_read, pattern_read, record_read,
                 records_read, bytes_read, port_readback, u.bits, event_count);
        $finish(0);
    end
endmodule
