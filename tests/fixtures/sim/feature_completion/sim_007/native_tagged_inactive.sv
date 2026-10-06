// SIM-007: inactive members of a tagged union with native members (SV 7.3.2,
// 11.9). Reads report a run-time error and yield the member type's default;
// writes report the error and store nothing.
typedef union tagged { int I; string S; real F; } value_t;

module tb;
    value_t x;

    initial begin
        $display("1 [%s]", x.S);
        x = tagged I 4;
        $display("2 [%s] %0.1f", x.S, x.F);
        x.S = "lost";
        x.I = 9;
        $display("3 %0d", x.I);
        x = tagged S "kept";
        x.I = 1;
        $display("4 %s", x.S);
        $finish(0);
    end
endmodule
