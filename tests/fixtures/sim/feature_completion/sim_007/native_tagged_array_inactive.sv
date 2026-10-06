// SIM-007: inactive members of tagged-union elements (SV 7.3.2, 11.9).
// Reads report a run-time error and yield the member type's default;
// writes report the error and store nothing.
typedef union tagged { void None; int I; string S; real F; } value_t;

module tb;
    value_t arr[2];
    value_t q[$];
    value_t a[int];

    initial begin
        arr[0] = tagged I 5;
        $display("1 [%s]", arr[0].S);
        arr[0].S = "lost";
        $display("2 %0d [%s] %0.1f", arr[0].I, arr[1].S, arr[1].F);
        q.push_back(tagged S "x");
        q[0].I = 3;
        a[4] = tagged F 1.5;
        $display("3 %s %0d", q[0].S, a[4].I);
        $finish(0);
    end
endmodule
