// A pattern binding is not visible in the false arm (SV 12.6.2).
module tb;
    typedef struct packed { logic [3:0] hi, lo; } pair_t;
    pair_t value;
    int result;
    initial begin
        value = 8'h12;
        if (value matches .only_true &&& only_true.hi == 0)
            result = only_true.lo;
        else
            result = only_true.lo;
    end
endmodule
