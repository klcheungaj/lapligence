// Automatic strings, class handles, virtual interfaces and events of
// procedural blocks that a `join_none` fork reads while the block runs
// again: each activation has its own variables, which its branches share
// (SV 6.21, 9.3.2). See readme.md for the expected lines.
class C;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

interface I;
    int m;
endinterface

module tb;
    I i0();
    I i1();
    int q[$] = '{1, 2, 3};
    logic clk = 0;
    int n = 0;

    task automatic show(ref string s, input C c);
        $display("show %s %0d %0d", s, s.len(), c.v);
        s = s.toupper();
    endtask

    // (A) A loop body's string, handle and virtual interface; the parent
    // writes the string after the fork, a branch writes all three later.
    initial begin
        i0.m = 5;
        i1.m = 6;
        foreach (q[k]) begin
            automatic string z = $sformatf("s%0d", q[k]);
            automatic C h = new(q[k] * 10);
            automatic virtual I v;
            if (k % 2) v = i1;
            else v = i0;
            fork
                #1 $display("A %s %0d %0d", z, h.v, v.m);
                begin
                    #2 z = {z, "!"};
                    h.v++;
                    v.m += 10;
                    $display("A' %s %0d %0d", z, h.v, v.m);
                end
            join_none
            z = "late";
        end
    end

    // (B) An always block's string, handle and event: each edge's branches
    // share that edge's variables.
    always #10 clk = ~clk;
    always @(posedge clk) begin
        automatic string s = $sformatf("c%0d", n);
        automatic C h = new(n);
        automatic event e;
        n++;
        fork
            begin
                @e;
                $display("B %s %0d woke", s, h.v);
            end
            begin
                #1 s = {s, "+"};
                h.v += 100;
                ->e;
            end
        join_none
    end

    // (C) A string declared in a branch that runs again, shared with the
    // branch's own nested fork.
    initial begin
        #20;
        repeat (2) begin
            fork
                begin
                    automatic string t = "t";
                    fork
                        #2 $display("C %s", t);
                    join_none
                    #1 t = "changed";
                end
            join_none
            #5;
        end
    end

    // (D) Waits on a shared handle and string, and a `ref` actual.
    initial begin
        #40;
        for (int i = 0; i < 2; i++) begin
            automatic string z = $sformatf("ab%0d", i);
            automatic C h = new(i);
            fork
                begin
                    @(h);
                    $display("D h changed %0d %s", h.v, z);
                    show(z, h);
                    $display("D after %s", z);
                end
                begin
                    wait (z == "go");
                    $display("D z is go %0d", i);
                end
            join_none
            #1 h = new(i + 50);
            #1 z = "go";
        end
    end

    // (E) A fork's own declaration in a loop.
    initial begin
        #50;
        for (int i = 0; i < 2; i++)
            fork
                automatic string name = $sformatf("w%0d", i);
                #1 $display("E %s", name);
            join_none
        #5 $finish(0);
    end
endmodule
