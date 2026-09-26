// IEEE 1800-2009 §11.9: an inner wrong tag diagnoses after a valid outer tag.
module tb;
    typedef union tagged packed { logic [7:0] A; logic [7:0] B; } inner_t;
    typedef union tagged packed { inner_t Data; logic [8:0] Other; } outer_t;
    outer_t values [0:1];
    logic [7:0] got;
    integer calls;
    function automatic int pick();
        calls = calls + 1;
        return 0;
    endfunction
    initial begin
        calls = 0;
        values[0] = tagged Data(tagged B(8'h5a));
        values[1] = tagged Other(9'h155);
        got = values[pick()].Data.A;
        $display("AFTER_NESTED_READ calls=%0d got=%h active_B=%h", calls, got, values[0].Data.B);
        $finish;
    end
endmodule
