// SIM-011 A03: the implementation and `this` of a running method are fixed
// when it is called (SV 8.20); rebinding the receiver variable during the
// method's wait affects only later calls.
class O;
    int id;
    function new(int i);
        id = i;
    endfunction
    virtual task hold(output int seen);
        #5;
        seen = id;
        id = id + 1;
    endtask
endclass

class P extends O;
    function new(int i);
        super.new(i);
    endfunction
    virtual task hold(output int seen);
        #5;
        seen = id * 100;
        id = id + 10;
    endtask
endclass

module tb;
    O h;
    O first;
    P p;
    int seen;

    initial begin
        h = new(1);
        first = h;
        fork
            h.hold(seen);
            begin
                #2;
                p = new(2);
                h = p;
            end
        join
        $display("t=%0d seen=%0d first=%0d h=%0d same=%0d", $time, seen, first.id, h.id, h == first);
        h.hold(seen);
        $display("t=%0d seen=%0d first=%0d h=%0d", $time, seen, first.id, h.id);
        $finish;
    end
endmodule
