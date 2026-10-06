// SIM-011 A01: a base-typed handle calls an overridden timed task that
// suspends, updates inherited packed, string and real properties and returns
// through output and ref arguments (SV 8.20, 13.3, 13.5).
class Base;
    int count;
    string tag;
    real scale;
    function new(string t);
        tag = t;
        scale = 1.5;
    endfunction
    virtual task run(input int n, output int o, ref int r);
        #1;
        o = n;
        r = r + 1;
    endtask
endclass

class Derived extends Base;
    int extra;
    function new();
        super.new("d");
        extra = 5;
    endfunction
    virtual task run(input int n, output int o, ref int r);
        #2;
        count = count + n;
        tag = {tag, "x"};
        o = n * 10 + extra;
        r = r + 100;
        #3 scale = scale * 2.0;
    endtask
endclass

// An override without timing behind a slot whose other overrides suspend.
class Quick extends Base;
    function new();
        super.new("q");
    endfunction
    virtual task run(input int n, output int o, ref int r);
        o = -n;
        r = r * 2;
    endtask
endclass

// `super.run` binds statically to Derived::run.
class Chain extends Derived;
    virtual task run(input int n, output int o, ref int r);
        super.run(n, o, r);
        #1 o = o + 1;
    endtask
endclass

module tb;
    Base b;
    Derived d;
    Quick q;
    Chain c;
    int o;
    int r;

    task automatic drive(Base h, input int n);
        int lo;
        h.run(n, lo, r);
        $display("drive t=%0d n=%0d lo=%0d r=%0d", $time, n, lo, r);
    endtask

    initial begin
        d = new;
        b = d;
        r = 7;
        b.run(4, o, r);
        $display("A t=%0d o=%0d r=%0d count=%0d tag=%s scale=%.2f", $time, o, r, d.count, d.tag, d.scale);
        q = new;
        b = q;
        b.run(3, o, r);
        $display("B t=%0d o=%0d r=%0d tag=%s", $time, o, r, b.tag);
        b = new("base");
        b.run(2, o, r);
        $display("C t=%0d o=%0d r=%0d tag=%s", $time, o, r, b.tag);
        c = new;
        b = c;
        drive(b, 1);
        $display("D t=%0d count=%0d tag=%s scale=%.2f", $time, c.count, c.tag, c.scale);
        $finish;
    end
endmodule
