// SIM-011 A03 nearest illegal form: a `protected` property is visible only
// in its class and subclasses (SV 8.18).
class O;
    protected int guarded;
endclass

class P extends O;
    function void set(int v);
        guarded = v;
    endfunction
endclass

module tb;
    P h;

    initial begin
        h = new;
        h.set(1);
        h.guarded = 2;
    end
endmodule
