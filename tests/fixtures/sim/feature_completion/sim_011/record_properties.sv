// SIM-011: a record class property with string and real members is one value
// per object (SV 7.2, 8.4, 8.8): per-object pattern initializer and default
// members, member writes inside methods (also after a wait), through handles
// and handle chains, whole-record copies to and from module records and
// equality.
typedef struct {
    string s;
    int n;
    real r;
} ns_t;

class Node;
    ns_t rec = '{s: "r", n: 2, r: 0.5};
    ns_t blank;
    Node next;
    function void bump();
        rec.n++;
        rec.s = {rec.s, "+"};
    endfunction
    function string tag();
        return $sformatf("%s%0d", rec.s, rec.n);
    endfunction
    task slow();
        #1 rec.r = rec.r * 4;
    endtask
endclass

module tb;
    Node n1;
    Node n2;
    ns_t tmp;

    initial begin
        n1 = new;
        n2 = new;
        n1.next = n2;
        n1.rec.n = n1.rec.n + 1;
        $display("%s %0d %.2f [%s] %0d", n1.rec.s, n1.rec.n, n1.rec.r, n1.blank.s, n1.blank.n);
        n1.bump();
        $display("%s %s", n1.tag(), n2.tag());
        n1.next.rec.s = "two";
        tmp = n1.rec;
        n2.blank = tmp;
        n2.slow();
        $display("t=%0d %s %s %0d %.2f %0d", $time, n2.rec.s, n2.blank.s, n2.blank.n, n2.rec.r,
                 n2.blank == n1.rec);
        $finish;
    end
endmodule
