// SIM-008: container and native record `ref` formals declared before or after
// input formals, in function and recursive timed-task calls (SV 13.5.2).
module tb;
    typedef struct { int a; string s; } r_t;
    function automatic void add(int n, ref int q[$]);
        q.push_back(n);
    endfunction
    function automatic void add2(ref int q[$], input int n);
        q.push_back(n);
    endfunction
    function automatic void set(int n, ref r_t r, input string t);
        r.a = n;
        r.s = t;
    endfunction
    function automatic int run();
        r_t l;
        set(3, l, "k");
        return l.a + l.s.len();
    endfunction
    task automatic rec(int n, ref int q[$]);
        int l[$];
        l.push_back(n);
        #1 q.push_back(n);
        if (n > 0) rec(n - 1, q);
        $display("%0d %0d", n, l[0]);
    endtask
    initial begin
        int q[$];
        int r[$];
        add(4, q);
        add2(q, 5);
        $display("%0d %0d %0d", q[0], q[1], run());
        rec(2, r);
        $display("size %0d at %0d", r.size(), $time);
    end
endmodule
