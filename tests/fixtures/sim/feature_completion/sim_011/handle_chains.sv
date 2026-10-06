// SIM-011: a property selected through handle properties uses the object
// the last handle names (SV 8.4), for packed, string and real properties,
// method calls and writes; static handle properties are one shared handle
// (SV 8.9); a real property without an initializer starts at 0.0 (Table
// 6-7); an initializer `= new` builds one object per instance (SV 8.8).
class Leaf;
    int v = 4;
endclass

class Node;
    int val;
    string name;
    real r;
    real unset;
    Node next;
    Leaf leaf = new;
    static Node head;
    function new(int v);
        val = v;
        name = $sformatf("n%0d", v);
        r = v * 0.5;
    endfunction
    function int get();
        return val;
    endfunction
endclass

module tb;
    Node n1;
    Node n2;
    Node n3;

    initial begin
        n1 = new(1);
        n2 = new(2);
        n3 = new(3);
        n1.next = n2;
        n2.next = n3;
        $display("%0d %0d %0d", n1.val, n1.next.val, n1.next.next.val);
        n1.next.val = 20;
        n1.next.next.val = 30;
        $display("%0d %0d %0d", n1.val, n2.val, n3.val);
        $display("%s %s %.1f %0d %0d", n1.next.name, n1.next.next.name, n1.next.r,
                 n1.next.get(), n1.next.next.get());
        n1.next.name = "two";
        n1.next.r = 9.5;
        $display("%s %.1f %.1f", n2.name, n2.r, n2.unset);
        Node::head = n2;
        $display("%0d %s", Node::head.val, Node::head.next.name);
        Node::head.val = 22;
        $display("%0d %0d %0d", n2.val, n1.leaf.v, n1.leaf == n2.leaf);
        n1.next.leaf.v = 44;
        $display("%0d %0d", n2.leaf.v, n1.leaf.v);
        $finish;
    end
endmodule
