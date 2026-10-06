// SIM-011: class-handle locals and formals of automatic subroutines select
// properties and call methods in the body, after a delay and in a joined
// fork branch (SV 8.4, 9.3.2, 13.3.1).
module tb;
    class pkt;
        int id;
        string nm;
        function new(int i);
            id = i;
            nm = "p";
        endfunction
        function int get();
            return id;
        endfunction
    endclass

    task automatic t();
        pkt p = new(1);
        $display("body id=%0d", p.id);
        #1 p.id = 2;
        $display("delay id=%0d %s %0d t=%0d", p.id, p.nm, p.get(), $time);
        fork
            begin
                #1 $display("fork id=%0d t=%0d", p.id, $time);
                p.id = 3;
            end
        join
        $display("joined id=%0d", p.id);
    endtask

    task automatic use_formal(pkt f);
        #1 $display("formal id=%0d t=%0d", f.id, $time);
        fork
            #1 f.nm = "q";
        join
    endtask

    function automatic int make();
        pkt q = new(7);
        q.id = q.id + 1;
        return q.id;
    endfunction

    pkt keep;

    initial begin
        t();
        keep = new(5);
        use_formal(keep);
        $display("f=%0d nm=%s t=%0d", make(), keep.nm, $time);
        $finish;
    end
endmodule
