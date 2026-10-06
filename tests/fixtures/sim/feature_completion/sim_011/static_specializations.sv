// SIM-011 A02: static properties and methods are shared per class
// specialization (SV 8.9, 8.10, 8.25): Counter#(4) and Counter#(8) are
// different classes with separate statics; a static task may suspend and
// may be called through the class scope or an instance.
class Counter #(int W = 4);
    static int created;
    static int total = 10;
    logic [W-1:0] v;
    function new();
        created++;
        v = '1;
    endfunction
    static function int get_created();
        return created;
    endfunction
    static task bump(int n);
        #1 total += n;
    endtask
endclass

module tb;
    Counter #(4) a1;
    Counter #(4) a2;
    Counter #(8) b1;

    initial begin
        a1 = new;
        a2 = new;
        b1 = new;
        Counter#(4)::bump(1);
        Counter#(8)::bump(5);
        fork
            Counter#(4)::bump(2);
            a2.bump(3);
        join
        $display("t=%0d c4=%0d c8=%0d tot4=%0d tot8=%0d v=%0d/%0d", $time,
                 Counter#(4)::get_created(), Counter#(8)::get_created(),
                 Counter#(4)::total, Counter#(8)::total, a1.v, b1.v);
        $finish;
    end
endmodule
