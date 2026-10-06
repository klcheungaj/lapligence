// SIM-007: strings, chandles, class handles and virtual interfaces declared
// in procedural blocks own one storage per declaration and instance, like
// module-scope variables: methods, formatting, comparisons, calls, class
// construction and methods, nonblocking writes and change markers.
interface bus_if;
    logic [3:0] d;
endinterface

class Obj;
    int v;
    string tag;
    function new(int x);
        v = x;
        tag = "o";
    endfunction
    function int get();
        return v;
    endfunction
    task bump();
        v++;
    endtask
endclass

class Base;
    virtual function string name();
        return "base";
    endfunction
endclass

class Der extends Base;
    virtual function string name();
        return "der";
    endfunction
endclass

typedef struct {
    string s;
    int k;
} node_t;

module tb;
    bus_if bi();
    node_t nd;
    string g = "G";
    int glen = g.len();
    int a = 1;
    int width;
    function automatic string up(string s);
        return s.toupper();
    endfunction
    task automatic app(inout string s, input string t);
        s = {s, t};
    endtask
    task automatic mk(output Obj o, input int x);
        o = new(x);
    endtask

    // A string declared in always_comb is written before it is read, so
    // the process waits only on `a`.
    always_comb begin
        string s;
        s = $sformatf("%0d", a);
        width = s.len();
    end

    initial begin : main
        static string t = "ab";
        static string st = {g, "1"};
        static chandle p = null;
        static Obj h = new(7), h2;
        Base b;
        Der d, d2;
        string u;
        virtual bus_if vi;
        $display("A %s %s %0d %s %0d", t, st, t.len(), up(t), glen);
        t.putc(0, "X");
        app(t, "cd");
        $display("B %s %s %0d %0d", t, t.substr(1, 2), t.compare("Xbcd"), t == "Xbcd");
        $sformat(u, "%s|%0d", t, h.get());
        $display("C %s", u);
        h.bump();
        h2 = h;
        $display("D %0d %0d %0d %0d", h2.v, h2 == h, h2 == null, p == null);
        mk(h2, 42);
        h2.tag = "renamed";
        $display("E %0d %0d %s %s", h2.get(), h.get(), h2.tag, h.tag);
        d = new;
        b = d;
        if ($cast(d2, b))
            $display("F %s %s", d2.name(), b.name());
        u = (t == "zz") ? "yes" : "no";
        u.itoa(123);
        $display("G %s %0d %s", u, u.atoi() + 1, {2{t}});
        vi = bi;
        vi.d = 4'h5;
        t <= "nba";
        #0 $display("H [%s] %h", t, bi.d);
        #1 $display("I [%s] %0d", t, width);
        a = 12345;
        #1 $display("J %0d", width);
    end

    // Readers of another process's static block variables wake on their
    // change markers and read them hierarchically.
    initial begin
        #2;
        @(main.t);
        $display("K %s at %0t", main.t, $time);
        @(main.h2);
        $display("L %0d at %0t", main.h2.v, $time);
        @(nd.s);
        $display("M %s at %0t", nd.s, $time);
    end
    initial begin
        #3 main.t = "w";
        #1 main.h2 = new(9);
        #1 nd.s = "m";
    end
    initial #10 $finish(0);
endmodule
