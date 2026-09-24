// llg-test-fixture: R13 recursive type keys through function return and aggregate port.
// R13 composition: recursive type keys survive an automatic return and value port.
// IEEE 1800-2009 10.9.1, 13.4, and 23.2.2.
typedef struct {
    int count;
    bit flag;
} record_t;
typedef record_t records_t [0:1];

module record_sum(input records_t records, output int sum);
    always_comb sum = records[0].count + records[1].count
                    + records[0].flag + records[1].flag;
endmodule

module tb;
    records_t records;
    int seed;
    int sum;

    function automatic records_t make_records(input int value);
        make_records = '{int: value, default: '0};
    endfunction

    record_sum sink(.records(records), .sum(sum));

    initial begin
        seed = 17;
        records = make_records(seed);
        #1;
        if (sum !== 34)
            $fatal(1, "recursive pattern function/port result: %0d", sum);
        $display("recursive pattern function/port passed: %0d", sum);
        $finish(0);
    end
endmodule
