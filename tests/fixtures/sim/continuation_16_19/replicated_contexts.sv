// SV 10.9.1: one replication dimension, positional order, context conversion.
module replication_check #(parameter W = 7) (output bit done);
    typedef logic signed [W-1:0] lane_t;
    typedef lane_t row_t [3:-2];
    typedef lane_t short_row_t [-1:0];
    typedef struct { lane_t a; lane_t b; } pair_t;
    typedef pair_t records_t [1:0];
    row_t row, returned, pending;
    lane_t cube [1:0][-1:1][2:1];
    lane_t many [16:0];
    lane_t singleton [0:0];
    logic [1:0][2:0][W-1:0] packed_rows;
    bit [W-1:0] two_state [1:0];
    records_t records;
    lane_t a, b;
    integer i, j, k;
    localparam row_t CONSTANT_ROW = '{3{lane_t'(1), lane_t'(0)}};

    function automatic row_t make_row(input lane_t x, input lane_t y);
        row_t local_row = '{3{x, y}};
        return local_row;
    endfunction

    function automatic bit same(input row_t x, input lane_t first, input lane_t second);
        for (int n = 3; n >= -2; n--)
            if (x[n] !== (((3-n)%2 == 0) ? first : second)) return 0;
        return 1;
    endfunction

    task automatic assign_row(output row_t target, input row_t value);
        target = value;
    endtask

    initial begin
        done = 0;
        a = '1;
        b = '0;
        row = '{3{a, b}};
        returned = make_row(a, b);
        if (!same(row, a, b) || !same(returned, a, b) ||
            !same(CONSTANT_ROW, lane_t'(1), lane_t'(0)))
            $fatal(1, "replication order, constant or automatic initializer");
        assign_row(returned, row_t'('{3{b, a}}));
        if (!same(returned, b, a)) $fatal(1, "typed replication actual");
        cube = '{2{'{3{'{a, b}}}}};
        for (i = 1; i >= 0; i--)
            for (j = -1; j <= 1; j++)
                if (cube[i][j][2] !== a || cube[i][j][1] !== b)
                    $fatal(1, "three dimensional replication");
        many = '{17{a}};
        singleton = '{1{b}};
        for (i = 16; i >= 0; i--)
            if (many[i] !== a) $fatal(1, "seventeen repeated positions");
        if (singleton[0] !== b) $fatal(1, "singleton pattern");
        packed_rows = '{2{'{3{a}}}};
        if (packed_rows !== {6{a}}) $fatal(1, "packed dimension ordering");
        records = '{2{'{a:a, b:b}}};
        if (records[1].a !== a || records[1].b !== b ||
            records[0].a !== a || records[0].b !== b)
            $fatal(1, "replication of record pattern");
        pending <= '{3{a, b}};
        a = '0;
        b = '1;
        #1;
        if (!same(pending, b, a)) $fatal(1, "replicated NBA issue snapshot");
        a = 'x;
        b = 'z;
        row = '{3{a, b}};
        two_state = '{2{a}};
        if (!same(row, a, b) || two_state[1] !== '0 || two_state[0] !== '0)
            $fatal(1, "replicated state conversion");
        done = 1;
    end
endmodule
module tb;
    wire [3:0] done;
    replication_check #(1) c1(done[0]);
    replication_check #(7) c7(done[1]);
    replication_check #(65) c65(done[2]);
    replication_check #(129) c129(done[3]);
    initial begin
        wait (&done);
        $display("REPLICATED_CONTEXTS_PASS");
        $finish(0);
    end
endmodule
