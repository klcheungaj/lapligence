// SIM-011: a shallow copy of a null handle is a run-time error (SV 8.4,
// 8.11) reported at the copy; the process does not continue.
class O;
    int id;
endclass

module tb;
    O h, c;

    initial begin
        $display("before");
        #1 c = new h;
        $display("after");
        $finish;
    end
endmodule
