module tb;
    typedef union packed {
        logic [15:0] raw;
        struct packed { logic [7:0] hi, lo; } halves;
    } word_t;
    typedef struct {
        word_t word;
        int values[1:-1];
        bit two;
        logic four;
    } record_t;
    record_t record;
    int calls;
    function automatic int index_once(); calls++; return 0; endfunction
    task automatic increment(ref int value); value += 7; endtask
    task automatic forward(ref record_t value);
        increment(value.values[index_once()]);
    endtask
    function automatic int observe(const ref int value); return value; endfunction
    function automatic int forward_read(const ref record_t value);
        return observe(value.values[0]);
    endfunction
    initial begin
        calls=0;
        record = '{word:16'h1234, values:'{1,2,3}, two:0, four:'x};
        record.word.halves.lo = 8'h5a;
        record.word.raw++;
        record.two = 1'bx;
        record.four = 1'bz;
        forward(record);
        if (forward_read(record) != 9) $fatal(1, "const member forwarding failed");
        $display("word=%h value=%0d states=%b,%b calls=%0d", record.word.raw, record.values[0], record.two, record.four, calls);
        $finish(0);
    end
endmodule
