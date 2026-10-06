// SIM-011: resizable-container and fixed unpacked-array class properties
// are per-object storage (SV 8.5, 7.4-7.10) with per-object initializers
// (SV 8.8), reachable inside the class's methods and through explicit
// handles, including handle chains and parameterized specializations.
class Bag;
    int q[$] = '{1, 2};
    int fixed[3] = '{7, 8, 9};
    logic [3:0] nib[2];
    string names[2] = '{"x", "y"};
    int aa[string];
    int d[];
    Bag next;
    function int total();
        int s = 0;
        foreach (fixed[i]) s += fixed[i];
        foreach (q[i]) s += q[i];
        return s;
    endfunction
    // Another object's property, selected through a formal handle.
    function void steal(Bag from);
        q.push_back(from.q.pop_front());
    endfunction
    task fill(int n);
        for (int i = 0; i < n; i++) begin
            #1 q.push_back(i * 10);
        end
    endtask
endclass

class Holder #(type T = int, int N = 2);
    T items[N];
    function void set(int i, T v);
        items[i] = v;
    endfunction
endclass

module tb;
    Bag a;
    Bag b;
    Holder #(byte, 3) p3;
    Holder #(string) ps;

    initial begin
        a = new;
        b = new;
        a.next = b;
        $display("%0d %0d %0d %0d %s%s", a.q.size(), a.q[1], a.fixed[2], $size(a.fixed),
                 a.names[0], a.names[1]);
        a.q.push_back(3);
        b.q.push_front(0);
        a.fixed[1] = 80;
        a.nib[1] = 4'hA;
        a.next.names[1] = "z";
        a.next.aa["k"] = 5;
        a.next.d = new[4];
        $display("%0d %0d %0d %h %h %s %0d %0d %0d", a.q.size(), b.q[0], a.total(), a.nib[1],
                 a.nib[0], b.names[1], b.aa["k"], b.d.size(), a.aa.num());
        a.fill(2);
        $display("t=%0d %0d %0d %0d", $time, a.q.size(), a.q[3], a.q[4]);
        b.steal(a);
        $display("%0d %0d %0d %0d", a.q.size(), a.q[0], b.q.size(), b.q[3]);
        p3 = new;
        ps = new;
        p3.set(2, 8'sd100);
        ps.set(1, "hi");
        $display("%0d %s %0d %0d", p3.items[2], ps.items[1], $size(p3.items), $size(ps.items));
        $finish;
    end
endmodule
