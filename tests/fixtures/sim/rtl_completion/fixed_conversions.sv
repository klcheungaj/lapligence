module tb;
    typedef struct { bit [7:0] two; logic [7:0] four; } record_t;
    typedef logic [7:0] narrow_t[2];
    typedef logic [7:0] wide_t[1:-0];
    narrow_t narrow_values;
    wide_t wide;
    record_t record[2];
    function automatic record_t convert(input logic [15:0] bits);
        return record_t'(bits);
    endfunction
    function automatic wide_t widen(input wide_t values);
        return values;
    endfunction
    initial begin
        narrow_values = '{8'h81,8'hf2};
        wide = widen(narrow_values);
        record[0] = convert(16'hxz);
        record[1] = record_t'(16'hxz);
        if (record[0].two !== 0 || record[1].two !== 0) $fatal(1,"two-state conversion");
        if (record[0].four !== 8'hxz || record[1].four !== 8'hxz) $fatal(1,"four-state conversion");
        $display("wide=%h,%h", wide[1], wide[0]);
        $finish(0);
    end
endmodule
