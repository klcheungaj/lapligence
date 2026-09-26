// IEEE 1800-2009 §11.9: an outer wrong tag diagnoses before a nested write.
module tb;
    typedef union tagged packed { logic [7:0] A; logic [7:0] B; } inner_t;
    typedef union tagged packed { inner_t Data; logic [8:0] Other; } outer_t;
    outer_t values [0:1];
    integer calls;
    function automatic int pick();
        calls = calls + 1;
        return 0;
    endfunction
    initial begin
        calls = 0;
        values[0] = tagged Other(9'h155);
        values[1] = tagged Data(tagged A(8'h11));
        values[pick()].Data.A = 8'h33;
        $display("AFTER_NESTED_WRITE calls=%0d active_Other=%h", calls, values[0].Other);
        $finish;
    end
endmodule
