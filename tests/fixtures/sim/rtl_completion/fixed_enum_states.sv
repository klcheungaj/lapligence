module tb;
    typedef enum bit [3:0] { B_ONE=1, B_TWO=2 } bit_enum_t;
    typedef enum integer { I_ONE=1, I_TWO=2 } integer_enum_t;
    typedef struct { bit_enum_t two; logic [3:0] four; } record_t;
    bit_enum_t two_values[2];
    integer_enum_t four_values[2];
    record_t records[2];
    function automatic record_t cast_record(input logic [7:0] value);
        return record_t'(value);
    endfunction
    function automatic record_t default_record();
    endfunction
    initial begin
        if (two_values[0] !== 4'h0 || four_values[0] !== 32'hx) $fatal(1,"enum array defaults");
        records[0] = cast_record(8'hxz);
        records[1] = default_record();
        if (records[0].two !== 4'h0 || records[0].four !== 4'hz) $fatal(1,"enum leaf conversion");
        if (records[1].two !== 4'h0 || records[1].four !== 4'hx) $fatal(1,"enum return defaults");
        $display("enum state domains passed");
        $finish(0);
    end
endmodule
