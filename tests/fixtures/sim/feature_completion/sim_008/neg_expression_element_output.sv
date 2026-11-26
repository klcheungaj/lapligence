// SIM-008 boundary: an output actual naming a queue element in a call of a
// string-returning function used inside an expression has no statement
// after the call to store the element, so it is rejected explicitly (legal
// by SV 13.5.2; a function with a packed result supports it).
module tb;
    int q[$];
    string s;

    function automatic string f(output int x);
        x = 4;
        return "r";
    endfunction

    initial begin
        q = '{0};
        s = {f(q[0]), "!"};
        $display("%s %0d", s, q[0]);
        $finish(0);
    end
endmodule
