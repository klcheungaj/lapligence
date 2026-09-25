// llg-test-fixture: SYN-013 static expression-call outputs have no copy-in/reset.
// IEEE 1800-2009 6.21, 13.4.2 and 13.5.1.
module output_case #(parameter W = 65, parameter SEED = 11)(output bit done);
    typedef logic signed [W-1:0] word_t;
    typedef word_t row_t[1:0];
    typedef struct { word_t data; bit present; } record_t;
    record_t records[2];
    row_t rows[2];
    word_t carry;
    int result, selected, selector_calls;
    int observed, actual;

    function automatic int pick();
        selector_calls++;
        return selected;
    endfunction
    function int retained(input bit write_it, output record_t record_value,
                          output row_t row_value, inout word_t accumulator);
        if (write_it) begin
            record_value = '{word_t'(SEED), 1'b1};
            row_value = '{word_t'(SEED + 1), word_t'(SEED + 2)};
        end
        accumulator = accumulator + word_t'(1);
        selected = 1;
        return int'(accumulator);
    endfunction
    function int default_reader(output int stored, input int value = stored,
                                input bit write_it = 0);
        if (write_it) stored = 12;
        return value;
    endfunction
    function automatic int fresh(output record_t record_value);
        return 0;
    endfunction
    task statement_retained(input bit write_it, output record_t record_value);
        if (write_it) record_value = '{word_t'(SEED + 3), 1'b1};
    endtask

    initial begin
        done = 0;
        selected = 0;
        selector_calls = 0;
        // A default value reaches every nested leaf recursively (SV 10.9.1).
        records = '{default:'0};
        rows = '{default:'0};
        carry = word_t'(40);
        result = retained(1, records[pick()], rows[0], carry);
        if (records[0].data !== word_t'(SEED) || records[0].present !== 1 ||
            records[1].present !== 0 || rows[0][1] !== word_t'(SEED + 1) ||
            rows[0][0] !== word_t'(SEED + 2) || carry !== word_t'(41) || result != 41)
            $fatal(1, "static output first call / selected capture");
        selected = 1;
        carry = word_t'(7);
        result = retained(0, records[pick()], rows[1], carry);
        if (records[1] !== records[0] || rows[1] !== rows[0] ||
            carry !== word_t'(8) || result != 8 || selector_calls != 2)
            $fatal(1, "retained output versus inout copy-in");
        actual = 99;
        observed = default_reader(actual, 0, 1);
        if (actual != 12 || observed != 0) $fatal(1, "default initialization control");
        actual = 77;
        observed = default_reader(actual);
        if (observed != 12 || actual != 12) $fatal(1, "default reads static formal");
        result = fresh(records[1]);
        if (records[1].data !== {W{1'bx}} || records[1].present !== 0)
            $fatal(1, "automatic output starts with typed default");
        statement_retained(1, records[0]);
        statement_retained(0, records[1]);
        if (records[0] !== records[1] || records[1].data !== word_t'(SEED + 3))
            $fatal(1, "statement-call persistence control");
        done = 1;
    end
endmodule
module tb;
    wire [3:0] done;
    output_case #(7, 11) a(done[0]);
    output_case #(65, 11) b(done[1]);
    output_case #(65, 21) c(done[2]);
    output_case #(129, 31) d(done[3]);
    initial begin
        #1;
        if (done !== 4'hf) $fatal(1, "incomplete output calls");
        $display("STATIC_OUTPUT_VALUES_PASS");
        $finish(0);
    end
endmodule
