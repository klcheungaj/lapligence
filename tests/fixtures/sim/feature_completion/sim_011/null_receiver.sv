// SIM-011 A03: calling a method through a null handle is a run-time error
// (SV 8.4) reported at the call; the process does not continue.
class O;
    int id;
    task t();
        #1 id = 1;
    endtask
endclass

module tb;
    O h;

    initial begin
        $display("before");
        #1 h.t();
        $display("after");
        $finish;
    end
endmodule
