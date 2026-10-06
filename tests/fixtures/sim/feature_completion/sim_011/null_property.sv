// SIM-011 A03: selecting a property through a null handle is a run-time
// error (SV 8.4) reported at the access; the process does not continue.
class O;
    int id;
endclass

module tb;
    O h;
    O g;

    initial begin
        g = new;
        g.id = 3;
        $display("before %0d", g.id);
        h.id = 5;
        $display("after");
        $finish;
    end
endmodule
