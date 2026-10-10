// SIM-040 negative: an open-array formal takes only fixed-size actuals in llg; a dynamic
// array actual is rejected before code generation (decision in readme).
module tb;
    import "DPI-C" function int neg_sum(input int a []);
    int d [];
    initial begin
        d = new[3];
        $display("%0d", neg_sum(d));
    end
endmodule
