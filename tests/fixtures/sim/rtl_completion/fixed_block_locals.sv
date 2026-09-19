module tb;
    typedef struct { int count; logic [7:0] data; } value_t;
    function automatic int sum(const ref value_t value);
        return value.count + int'(value.data);
    endfunction
    initial begin
        repeat (2) begin
            automatic value_t record = '{count:3, data:8'h12};
            automatic int values[1:-1] = '{4,5,6};
            static value_t saved = '{count:10, data:8'h20};
            record.count += values[0];
            saved.count++;
            $display("local=%0d saved=%0d", sum(record), saved.count);
        end
        $finish(0);
    end
endmodule
