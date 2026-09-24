// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/record_reduction_init.sv
// IEEE 1800-2009 §§6.21, 7.12.3, and 13.5.1: a reduction over an unpacked
// record-array input can initialize an automatic local in a zero-time task.
module tb;
    typedef struct {
        logic [7:0] data;
    } record_t;
    typedef record_t records_t [0:1];

    task automatic sum_records(input records_t source);
        automatic int total = source.sum() with (int'(item.data));

        if (total !== 10)
            $fatal(1, "fixed record reduction initializer: got %0d", total);
        $display("total=%0d", total);
    endtask

    initial begin
        records_t source;
        source[0].data = 8'd3;
        source[1].data = 8'd7;
        sum_records(source);
        $finish(0);
    end
endmodule
