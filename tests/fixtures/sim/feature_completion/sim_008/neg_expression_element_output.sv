// SIM-008 boundary: an output actual naming a string element of a queue in a
// call used inside an expression has no string storage of its own for the
// call to write, so it is rejected explicitly (legal by SV 13.5.2; a call
// statement supports it, and an expression call supports packed and real
// elements).
module tb;
    string q[$];
    string s;

    function automatic string f(output string x);
        x = "w";
        return "r";
    endfunction

    initial begin
        q = '{"a"};
        s = {f(q[0]), "!"};
        $display("%s %s", s, q[0]);
        $finish(0);
    end
endmodule
