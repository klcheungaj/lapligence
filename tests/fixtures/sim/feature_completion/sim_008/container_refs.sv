// SIM-008: `ref` formals of queue and associative-array type alias the
// caller's container (SV 13.5.2): writes are visible to the caller, to
// always_comb readers and to a second ref of the same container at once.
module tb;
    int q[$];
    int aa[string];
    string sq[$];
    int seen;
    always_comb seen = q.size();
    task automatic qref(ref int qq[$], input int v);
        qq.push_back(v);
        #1;
        qq[0] = qq.size();
    endtask
    function automatic void sref(ref string s[$]);
        s.push_front("z");
    endfunction
    function automatic int both(ref int a[$], ref int b[$]);
        a.push_back(1);
        return b.size();
    endfunction
    function automatic void aref(ref int m[string]);
        m["k"] = 3;
    endfunction
    initial begin
        q = '{5};
        fork qref(q, 9); join_none
        #0 $display("%0d %0d %0d", q.size(), q[1], seen);
        #2 $display("%0d", q[0]);
        sq = '{"a"}; sref(sq); $display("%0d %s", sq.size(), sq[0]);
        $display("%0d", both(q, q));
        aref(aa); $display("%0d", aa["k"]);
        $finish(0);
    end
endmodule
