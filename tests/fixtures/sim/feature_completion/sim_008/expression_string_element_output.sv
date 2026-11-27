// SIM-008, decision EO-D3: a string element of a queue or an associative
// array as an output or inout actual of a call inside an expression copies
// out like a packed element: the call writes a caller temporary, stored
// into the element, located again, when the call returns (SV 13.5.2), before
// the rest of the expression. An inout actual of a missing associative entry
// copies in the default value and creates the entry at the copy-out.
module tb;
    string q[$];
    string aa[int];
    string s;
    int n;

    function automatic string f(output string x);
        x = "w";
        return "r";
    endfunction

    function automatic int g(inout string x);
        x = {x, "+"};
        return 7;
    endfunction

    initial begin
        q = '{"a", "b"};
        aa[3] = "c";
        s = {f(q[0]), "!"};
        $display("%s %s", s, q[0]);
        n = g(q[1]) + 1;
        $display("%0d %s", n, q[1]);
        s = {f(aa[3]), "?"};
        $display("%s %s", s, aa[3]);
        n = g(aa[5]) * 2;
        $display("%0d %s %0d", n, aa[5], aa.num());
        $finish(0);
    end
endmodule
