// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_aggregate_lvalue_matrix.sv
// IEEE 1800-2009 §§6.8, 7.4, and 10.5: typed fixed-array, record, and enum
// targets retain their selected row, concatenation, and positional-pattern writes.
module tb;
    typedef logic [7:0] byte_t;
    typedef enum logic [1:0] { LOW = 2'b01, HIGH = 2'b10 } state_t;
    typedef state_t states_t [0:1];
    typedef struct { byte_t code; byte_t data; } record_t;
    typedef record_t records_t [0:1];
    typedef struct { byte_t code; byte_t lanes [0:1]; } payload_t;

    records_t record_target = '{
        0: '{code:8'h10, data:8'h11},
        1: '{code:8'h12, data:8'h13}
    };
    records_t record_source = '{
        0: '{code:8'h20, data:8'h21},
        1: '{code:8'h22, data:8'h23}
    };
    payload_t payload_target = '{
        code:8'h30,
        lanes:'{0:8'h31, 1:8'h32}
    };
    states_t state_source = '{LOW, HIGH};
    state_t state_low, state_high;

    initial begin
        // The focal slot is records_t, and the lvalue is its selected row.
        record_target[0:1] = '{
            0: '{code:8'h40, data:8'h41},
            1: '{code:8'h42, data:8'h43}
        };
        if (record_target[0].code !== 8'h40 || record_target[0].data !== 8'h41 ||
            record_target[1].code !== 8'h42 || record_target[1].data !== 8'h43)
            $fatal(1, "fixed-array record row-slice write mismatch");

        // Both concatenation leaves project from the same records_t target.
        {record_target[0].code, record_target[1].data} = {8'h50, 8'h51};
        if (record_target[0].code !== 8'h50 || record_target[0].data !== 8'h41 ||
            record_target[1].code !== 8'h42 || record_target[1].data !== 8'h51)
            $fatal(1, "fixed-array record concatenation write mismatch");

        records_t'{record_target[0], record_target[1]} = record_source;
        if (record_target[0].code !== 8'h20 || record_target[0].data !== 8'h21 ||
            record_target[1].code !== 8'h22 || record_target[1].data !== 8'h23)
            $fatal(1, "fixed-array record positional-pattern write mismatch");

        // The focal slot is payload_target; the selected address is its row.
        payload_target.lanes[0:1] = '{0:8'h60, 1:8'h61};
        if (payload_target.code !== 8'h30 || payload_target.lanes[0] !== 8'h60 ||
            payload_target.lanes[1] !== 8'h61)
            $fatal(1, "unpacked-record row-slice write mismatch");

        {payload_target.code, payload_target.lanes[0]} = {8'h70, 8'h71};
        if (payload_target.code !== 8'h70 || payload_target.lanes[0] !== 8'h71 ||
            payload_target.lanes[1] !== 8'h61)
            $fatal(1, "unpacked-record concatenation write mismatch");

        // The positional pattern writes two focal state_t enum slots.
        states_t'{state_low, state_high} = state_source;
        if (state_low !== LOW || state_high !== HIGH)
            $fatal(1, "enum positional-pattern write mismatch");

        $display("records=%h,%h,%h,%h payload=%h,%h,%h enum=%h,%h",
                 record_target[0].code, record_target[0].data,
                 record_target[1].code, record_target[1].data,
                 payload_target.code, payload_target.lanes[0], payload_target.lanes[1],
                 state_low, state_high);
        $finish(0);
    end
endmodule
