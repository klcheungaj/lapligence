// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_formal_matrix.sv
module tb;
    typedef enum logic [7:0] { IDLE = 8'h00, ACTIVE = 8'h01 } state_t;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    typedef union packed { logic [15:0] word; pair_t halves; } union_t;
    typedef struct { logic [7:0] key; logic [7:0] payload; } record_t;
    typedef record_t records_t [0:1];
    typedef struct { logic [7:0] code; logic [7:0] payload; } unpacked_t;

    task automatic set_enum_output(output state_t value);
        value = ACTIVE;
    endtask
    task automatic set_enum_inout(inout state_t value);
        value = ACTIVE;
    endtask
    task automatic set_enum_ref(ref state_t value);
        value = ACTIVE;
    endtask
    task automatic check_enum_const(const ref state_t value);
        if (value !== ACTIVE) $fatal(1, "enum const-ref actual mismatch");
    endtask

    task automatic set_pair_inout(inout pair_t value);
        value.hi = 8'h5a;
        value.lo = 8'ha5;
    endtask

    task automatic set_union_inout(inout union_t value);
        value.word = 16'h5aa5;
    endtask
    task automatic check_union_const(const ref union_t value);
        if (value.word !== 16'hb5c6) $fatal(1, "union const-ref actual mismatch");
    endtask

    task automatic make_records(output records_t value);
        value[0] = '{key:8'h11, payload:8'ha1};
        value[1] = '{key:8'h22, payload:8'hb2};
    endtask
    task automatic set_records_inout(inout records_t value);
        value[0].payload = 8'hc3;
        value[1].key = 8'h44;
    endtask
    task automatic check_records_const(const ref records_t value);
        if (value[0].key !== 8'h55 || value[0].payload !== 8'h66 ||
            value[1].key !== 8'h77 || value[1].payload !== 8'h88)
            $fatal(1, "record array const-ref actual mismatch");
    endtask

    task automatic set_unpacked_ref(ref unpacked_t value);
        value.payload = 8'hc7;
    endtask

    state_t enum_out;
    state_t enum_inout = IDLE;
    state_t enum_ref = IDLE;
    state_t enum_const = ACTIVE;
    pair_t packed_inout = '{hi:8'h12, lo:8'h34};
    union_t union_inout = union_t'(16'h1234);
    union_t union_const = union_t'(16'hb5c6);
    records_t records_out;
    records_t records_inout = '{0:'{key:8'h31,payload:8'h41}, 1:'{key:8'h32,payload:8'h42}};
    records_t records_const = '{0:'{key:8'h55,payload:8'h66}, 1:'{key:8'h77,payload:8'h88}};
    unpacked_t unpacked_ref = '{code:8'h31, payload:8'h32};

    initial begin
        set_enum_output(enum_out);
        set_enum_inout(enum_inout);
        set_enum_ref(enum_ref);
        check_enum_const(enum_const);
        set_pair_inout(packed_inout);
        set_union_inout(union_inout);
        check_union_const(union_const);
        make_records(records_out);
        set_records_inout(records_inout);
        check_records_const(records_const);
        set_unpacked_ref(unpacked_ref);

        if (enum_out !== ACTIVE || enum_inout !== ACTIVE || enum_ref !== ACTIVE ||
            packed_inout.hi !== 8'h5a || packed_inout.lo !== 8'ha5 || union_inout.word !== 16'h5aa5 ||
            records_out[0].key !== 8'h11 || records_out[0].payload !== 8'ha1 ||
            records_out[1].key !== 8'h22 || records_out[1].payload !== 8'hb2 ||
            records_inout[0].payload !== 8'hc3 || records_inout[1].key !== 8'h44 ||
            unpacked_ref.code !== 8'h31 || unpacked_ref.payload !== 8'hc7)
            $fatal(1, "typed formal matrix mismatch");

        $display("enum=%h,%h,%h pair=%h union=%h records=%h/%h,%h/%h inout=%h/%h ref=%h/%h",
                 enum_out, enum_inout, enum_ref, packed_inout, union_inout.word,
                 records_out[0].key, records_out[0].payload,
                 records_out[1].key, records_out[1].payload,
                 records_inout[0].payload, records_inout[1].key,
                 unpacked_ref.code, unpacked_ref.payload);
        $finish(0);
    end
endmodule
