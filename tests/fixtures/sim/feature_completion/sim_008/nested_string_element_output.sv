// SIM-008, decision EO-D3: a string element of a queue or dynamic array
// nested in an element of a queue, dynamic or associative array (`sq[0][1]`)
// as an output or inout actual, of a task call statement and of a function
// called inside an expression, copies out like an element of a
// one-dimensional queue: the call writes a caller temporary, stored into the
// element, located again, when the call returns (SV 13.5.2). An inout actual
// after a call that changes its index copies in and out the element that
// index selects then.
module tb;
    string sq[$][$];
    string da[][];
    string aq[int][$];
    string qa[$][string];
    string s3[$][$][$];
    string s;
    int k, n;

    function automatic string f(output string o);
        o = "w";
        return "r";
    endfunction

    function automatic int g(inout string o);
        o = {o, "+"};
        return 7;
    endfunction

    task automatic t(output string o);
        o = "v";
    endtask

    task automatic u(inout string o);
        #1 o = {o, "-"};
    endtask

    function int bump();
        k = 1;
        return 1;
    endfunction

    initial begin
        sq = '{'{"a", "b"}, '{"c"}};
        da = new[2];
        da[0] = new[2];
        da[1] = new[1];
        da[0][0] = "d0";
        da[0][1] = "d1";
        da[1][0] = "e0";
        aq[3] = '{"x", "y"};
        qa = '{'{"k": "a"}};
        s3 = '{'{'{"m", "n"}}};
        t(sq[0][1]);
        $display("t %p", sq);
        u(sq[1][0]);
        $display("u %p", sq);
        s = {f(sq[0][0]), "!"};
        $display("f %s %p", s, sq);
        n = g(sq[0][1]) + 1;
        $display("g %0d %p", n, sq);
        t(da[1][0]);
        n = g(da[0][1]) * 2;
        $display("da %0d %p", n, da);
        t(aq[3][1]);
        s = {f(aq[3][0]), "?"};
        $display("aq %s %s %s", s, aq[3][0], aq[3][1]);
        n = g(aq[3][1]) + 2;
        $display("aq %0d %s", n, aq[3][1]);
        t(qa[0]["k"]);
        $display("qa %s", qa[0]["k"]);
        t(s3[0][0][1]);
        n = g(s3[0][0][0]);
        $display("s3 %0d %p", n, s3);
        k = 0;
        n = bump() + g(sq[k][0]);
        $display("k %0d %p", n, sq);
        $finish(0);
    end
endmodule
