// IEEE 1800-2009 7.12.2: fixed-array sort and rsort preserve immediate
// elements, declared bounds, and side-effect-free with-clause map keys.
module tb;
    typedef struct packed {
        logic [7:0] key;
        logic [7:0] payload;
    } record_t;
    typedef enum logic [1:0] {
        ENUM_ZERO = 2'd0,
        ENUM_ONE = 2'd1,
        ENUM_TWO = 2'd2
    } enum_t;

    int signed_values [3:0];
    logic [7:0] unsigned_values [0:3];
    record_t records [0:3];
    enum_t enum_values [0:2];
    logic [7:0] singleton [7:7];

    task automatic sort_signed(ref int values [3:0]);
        values.sort();
    endtask

    task automatic sort_records(ref record_t values [0:3]);
        values.sort() with (item.key);
    endtask

    initial begin
        signed_values[3] = 32'sd7;
        signed_values[2] = -32'sd2;
        signed_values[1] = 32'sd4;
        signed_values[0] = 32'sd1;
        sort_signed(signed_values);
        if (signed_values[3] !== -32'sd2
                || signed_values[2] !== 32'sd1
                || signed_values[1] !== 32'sd4
                || signed_values[0] !== 32'sd7) begin
            $display("FAIL syn_028_signed");
            $finish;
        end

        unsigned_values[0] = 8'd3;
        unsigned_values[1] = 8'd1;
        unsigned_values[2] = 8'd255;
        unsigned_values[3] = 8'd0;
        unsigned_values.rsort();
        if (unsigned_values[0] !== 8'd255
                || unsigned_values[1] !== 8'd3
                || unsigned_values[2] !== 8'd1
                || unsigned_values[3] !== 8'd0) begin
            $display("FAIL syn_028_unsigned");
            $finish;
        end
        unsigned_values.sort();
        if (unsigned_values[0] !== 8'd0
                || unsigned_values[1] !== 8'd1
                || unsigned_values[2] !== 8'd3
                || unsigned_values[3] !== 8'd255) begin
            $display("FAIL syn_028_unsigned_sort");
            $finish;
        end

        records[0] = '{key: 8'd2, payload: 8'ha0};
        records[1] = '{key: 8'd1, payload: 8'ha1};
        records[2] = '{key: 8'd2, payload: 8'ha2};
        records[3] = '{key: 8'd0, payload: 8'ha3};
        sort_records(records);
        if (records[0].key !== 8'd0 || records[1].key !== 8'd1
                || records[2].key !== 8'd2 || records[3].key !== 8'd2) begin
            $display("FAIL syn_028_record_sort");
            $finish;
        end
        if (records[0].payload === records[1].payload
                || records[0].payload === records[2].payload
                || records[0].payload === records[3].payload
                || records[1].payload === records[2].payload
                || records[1].payload === records[3].payload
                || records[2].payload === records[3].payload
                || (records[0].payload !== 8'ha0 && records[0].payload !== 8'ha1
                    && records[0].payload !== 8'ha2 && records[0].payload !== 8'ha3)
                || (records[1].payload !== 8'ha0 && records[1].payload !== 8'ha1
                    && records[1].payload !== 8'ha2 && records[1].payload !== 8'ha3)
                || (records[2].payload !== 8'ha0 && records[2].payload !== 8'ha1
                    && records[2].payload !== 8'ha2 && records[2].payload !== 8'ha3)
                || (records[3].payload !== 8'ha0 && records[3].payload !== 8'ha1
                    && records[3].payload !== 8'ha2 && records[3].payload !== 8'ha3)) begin
            $display("FAIL syn_028_record_sort_payloads");
            $finish;
        end
        records.rsort() with (item.key);
        if (records[0].key !== 8'd2 || records[1].key !== 8'd2
                || records[2].key !== 8'd1 || records[3].key !== 8'd0) begin
            $display("FAIL syn_028_record_rsort");
            $finish;
        end
        if (records[0].payload === records[1].payload
                || records[0].payload === records[2].payload
                || records[0].payload === records[3].payload
                || records[1].payload === records[2].payload
                || records[1].payload === records[3].payload
                || records[2].payload === records[3].payload
                || (records[0].payload !== 8'ha0 && records[0].payload !== 8'ha1
                    && records[0].payload !== 8'ha2 && records[0].payload !== 8'ha3)
                || (records[1].payload !== 8'ha0 && records[1].payload !== 8'ha1
                    && records[1].payload !== 8'ha2 && records[1].payload !== 8'ha3)
                || (records[2].payload !== 8'ha0 && records[2].payload !== 8'ha1
                    && records[2].payload !== 8'ha2 && records[2].payload !== 8'ha3)
                || (records[3].payload !== 8'ha0 && records[3].payload !== 8'ha1
                    && records[3].payload !== 8'ha2 && records[3].payload !== 8'ha3)) begin
            $display("FAIL syn_028_record_rsort_payloads");
            $finish;
        end

        enum_values[0] = ENUM_TWO;
        enum_values[1] = ENUM_ZERO;
        enum_values[2] = ENUM_ONE;
        enum_values.sort();
        if (enum_values[0] !== ENUM_ZERO
                || enum_values[1] !== ENUM_ONE
                || enum_values[2] !== ENUM_TWO) begin
            $display("FAIL syn_028_enum");
            $finish;
        end

        singleton[7] = 8'd42;
        singleton.sort();
        singleton.rsort();
        if (singleton[7] !== 8'd42) begin
            $display("FAIL syn_028_singleton");
            $finish;
        end

        $display("PASS syn_028_fixed_sort");
        $finish;
    end
endmodule
