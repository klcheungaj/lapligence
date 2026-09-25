// llg-test-fixture: SYN-006 source dependencies, typed rows, net sites and disjoint writers.
module row_relay #(parameter W = 65)(
    input wire [W-1:0] source[-1:1], output wire [W-1:0] result[-1:1]
);
    assign result = source;
endmodule
module continuous_case #(parameter W = 65)(output bit done);
    typedef logic [W-1:0] lane_t;
    typedef lane_t row_t[-1:1];
    typedef bit [W-1:0] bit_row_t[-1:1];
    typedef struct { lane_t data; bit valid; } record_t;
    row_t a = '{'1, '0, '1};
    row_t b = '{'1, lane_t'(1), '1};
    logic choice = 1'bx;
    wire [W-1:0] resolved[1:-1];
    wire [W-1:0] conditional[-1:1];
    wire [W-1:0] relayed[-1:1];
    wire [W-1:0] window[3:0];
    lane_t copied[-1:1];
    bit_row_t converted;
    lane_t disjoint[2];
    lane_t expected;
    record_t record_a[2] = '{'{lane_t'(1), 1'b1}, '{lane_t'(1), 1'b1}};
    record_t record_b[2] = '{'{lane_t'(1), 1'b1}, '{lane_t'(0), 1'b1}};
    record_t record_result[2];

    assign resolved = a;
    assign resolved = b;
    assign conditional = choice ? a : b;
    assign copied = a;
    // Unpacked element types must be equivalent (SV 7.6); the four-to-two
    // state conversion is an explicit bit-stream cast (SV 6.24.3).
    assign converted = bit_row_t'(a);
    assign window[3:2] = a[-1:0];
    assign disjoint[0] = a[-1];
    assign record_result = choice ? record_a : record_b;
    row_relay #(W) relay(a, relayed);

    initial begin
        done = 0;
        disjoint[1] = lane_t'(1);
        #1;
        expected = '0;
        expected[0] = 1'bx;
        if (resolved[1] !== '1 || resolved[0] !== expected || resolved[-1] !== '1)
            $fatal(1, "independent net drivers or range correspondence");
        if (conditional[-1] !== '1 || conditional[0] !== {W{1'bx}} || conditional[1] !== '1)
            $fatal(1, "array conditional uses whole immediate elements");
        if (copied !== a || relayed !== a || window[3] !== a[-1] || window[2] !== a[0] ||
            window[1] !== {W{1'bz}} || window[0] !== {W{1'bz}})
            $fatal(1, "continuous values, links or slice");
        if (record_result[0] !== record_a[0] || record_result[1].data !== {W{1'bx}} ||
            record_result[1].valid !== 0) $fatal(1, "record-array element default");
        choice = 1;
        a[1] = 'z;
        b[0] = 'z;
        #1;
        if (resolved[0] !== '0 || resolved[-1] !== '1 || copied[1] !== {W{1'bz}} ||
            converted[1] !== '0 || relayed[1] !== {W{1'bz}})
            $fatal(1, "content dependency, state conversion or driver release");
        if (record_result !== record_a) $fatal(1, "known record array choice");
        record_a[0].valid = 0;
        a[-1] = lane_t'(1);
        #1;
        if (record_result[0].valid !== 0 || copied[-1] !== lane_t'(1) ||
            disjoint[0] !== lane_t'(1) || disjoint[1] !== lane_t'(1))
            $fatal(1, "nested/member content or independent writer");
        choice = 0;
        b[1] = 'z;
        #1;
        if (conditional !== b || record_result !== record_b || resolved[-1] !== {W{1'bz}})
            $fatal(1, "selector dependency or all drivers released");
        done = 1;
    end
endmodule
module tb;
    wire [3:0] done;
    continuous_case #(1) a(done[0]);
    continuous_case #(7) b(done[1]);
    continuous_case #(65) c(done[2]);
    continuous_case #(129) d(done[3]);
    initial begin
        #5;
        if (done !== 4'hf) $fatal(1, "incomplete continuous cases");
        $display("CONTINUOUS_CONTEXTS_PASS");
        $finish(0);
    end
endmodule
