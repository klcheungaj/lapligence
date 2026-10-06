// SIM-007: lifetimes of strings and handles declared in procedural blocks
// (SV 6.21): static ones keep their value and initialize once before the
// processes start; automatic ones restart from the empty string or null and
// run their initializer at every entry of their block.
class Obj;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

module leaf #(parameter string NAME = "?", parameter int D = 1) ();
    initial begin
        static string tag = {NAME, "!"};
        #D $display("I %s %0d", tag, D);
    end
endmodule

module tb;
    int calls;
    Obj keep[$];
    bit clk;
    function automatic string next_name();
        calls++;
        return $sformatf("n%0d", calls);
    endfunction

    leaf #("one", 8) u1();
    leaf #("two", 9) u2();
    for (genvar gi = 0; gi < 2; gi++) begin : g
        initial begin
            string loc;
            loc = $sformatf("g%0d", gi);
            #(10 + gi) $display("G %s", loc);
        end
    end

    initial begin
        // (A) static vs automatic over loop re-entry
        for (int i = 0; i < 3; i++) begin
            static string st;
            static Obj sh;
            automatic string au;
            automatic Obj ah;
            automatic chandle ac;
            $display("A [%s] %0d [%s] %0d %0d", st, sh == null, au, ah == null, ac == null);
            st = {st, "s"};
            if (sh == null) sh = new(i);
            au = "a";
            ah = new(10 + i);
            keep.push_back(ah);
        end
        $display("A %0d %0d %0d", keep.size(), keep[0].v, keep[2].v);
        // (B) a static initializer runs once in the static schedule; an
        // automatic one at every entry
        for (int i = 0; i < 2; i++) begin
            static string si = next_name();
            automatic string ai = next_name();
            $display("B %s %s %0d", si, ai, calls);
        end
        // (C) an automatic string in a fork ... join branch is reset at each
        // entry; both branches of one fork share the parent's variable
        for (int i = 0; i < 2; i++) begin
            automatic string shared;
            fork
                begin
                    automatic string own;
                    $display("C [%s]", own);
                    own = "x";
                    shared = {shared, "1"};
                end
                begin
                    #1 shared = {shared, "2"};
                end
            join
            $display("C %s", shared);
        end
        // (D) a join_none fork in a loop that does not read the variable
        for (int i = 0; i < 2; i++) begin
            automatic string t = $sformatf("t%0d", i);
            fork
                #1 $display("D child");
            join_none
            $display("D %s", t);
        end
        // (E) a fork that runs once keeps its own declarations and a block
        // that runs once keeps its variable for a branch still running
        fork
            automatic string fs = "fork";
            #3 $display("E %s", fs);
        join_none
        begin
            automatic Obj late = new(77);
            fork
                #4 $display("E %0d", late.v);
            join_none
        end
        #5 $display("E done");
    end

    // (F) a static string in a named block of an always procedure keeps
    // accumulating; an automatic one restarts
    always @(posedge clk) begin : edge_blk
        static string acc;
        automatic string fresh;
        acc = {acc, "+"};
        fresh = {fresh, "+"};
        $display("F %s %s", acc, fresh);
    end
    initial begin
        #12 clk = 1;
        #1 clk = 0;
        #1 clk = 1;
        #1 $display("F %s", edge_blk.acc);
    end
    // (H) a same-named variable in another process is separate storage
    initial begin
        string st;
        #16 $display("H [%s]", st);
        $finish(0);
    end
endmodule
