module tb;
    typedef struct { int count; logic [7:0] data; } value_t;
    int calls = 0;
    function automatic value_t make(input int count);
        calls++;
        return '{count:count, data:8'ha5};
    endfunction
    value_t record = make(7);
    value_t elements[2] = '{make(9), make(11)};
    initial begin
        if (calls != 3) $fatal(1,"initializer calls precede processes");
        if (record.count != 7 || elements[0].count != 9 || elements[1].count != 11)
            $fatal(1,"fixed initialization before processes");
        $display("calls=%0d values=%0d,%0d,%0d", calls, record.count, elements[0].count, elements[1].count);
        $finish(0);
    end
endmodule
