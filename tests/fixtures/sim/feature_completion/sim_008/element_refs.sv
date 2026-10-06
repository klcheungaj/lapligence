// SIM-008: `ref` actuals that select dynamic-array elements, associative
// entries and queue elements retain the element (SV 13.5.2, 7.10.3). The
// reference sees and makes immediate changes until an operation outdates it;
// an outdated element keeps its last value, private to the subroutines that
// still reference it.
module tb;
    int d[];
    int aa[int];
    int sa[string];
    int q[$];
    int seen;
    always_comb seen = d.size() > 1 ? d[1] : -1;

    task automatic hold_dyn(ref int x);
        #2 $display("dyn live %0d %0d", x, seen);
        x = 6;
        #0 $display("dyn write %0d %0d", d[1], seen);
        #2 x = 9;
        $display("dyn outdated %0d %0d %0d", x, d[1], d.size());
    endtask

    task automatic hold_assoc(ref int x);
        #2 $display("assoc live %0d", x);
        x = 8;
        $display("assoc write %0d", aa[3]);
        #2 x = 11;
        $display("assoc outdated %0d %0d", x, aa.exists(3));
    endtask

    function automatic void put(ref int x);
        x = 4;
    endfunction

    task automatic pair(ref int a, ref int b);
        #2 a = 3;
        $display("pair %0d %0d", b, d.size());
    endtask

    task automatic follow(ref int x);
        #2 x = 99;
    endtask

    function automatic int poke(ref int x);
        x = 1;
        return x;
    endfunction

    initial begin
        d = '{1, 2, 3};
        fork
            hold_dyn(d[1]);
        join_none
        #1 d[1] = 5;
        #2 d = new[5](d);
        #7;
        aa[3] = 1;
        fork
            hold_assoc(aa[3]);
        join_none
        #1 aa[3] = 7;
        #2 aa.delete(3);
        #7;
        put(sa["new"]);
        $display("string %0d %0d", sa.exists("new"), sa["new"]);
        fork
            pair(d[0], d[0]);
        join_none
        #1 d.delete();
        #2;
        q = '{10, 20, 30};
        fork
            follow(q[2]);
        join_none
        #1 q.delete(0);
        #2 $display("queue %0d %0d", q.size(), q[1]);
        $display("invalid %0d %0d", poke(d[0]), d.size());
    end
endmodule
