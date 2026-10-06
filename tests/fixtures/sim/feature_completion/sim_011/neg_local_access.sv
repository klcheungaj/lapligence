// SIM-011 A03 nearest illegal form: a `local` property is not visible
// outside its class (SV 8.18).
class O;
    local int secret;
    function int peek();
        return secret;
    endfunction
endclass

module tb;
    O h;

    initial begin
        h = new;
        h.secret = 1;
        $display("%0d", h.peek());
    end
endmodule
