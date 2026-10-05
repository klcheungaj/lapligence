// SIM-007: records whose members are class handles copy the handle only
// (SV 7.2, 8.4); the designated object is shared, never copied.
class counter_c;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

typedef struct { string name; real w; counter_c h; } item_t;

module tb;
    item_t a, b;
    item_t list[3];

    function automatic item_t bump(input item_t x);
        x.name = {x.name, "+"};
        x.h.v++;
        return x;
    endfunction

    task automatic rename(inout item_t x, input string n);
        x.name = n;
    endtask

    initial begin
        a.name = "a";
        a.w = 0.25;
        a.h = new(10);
        b = bump(a);
        $display("1 %s %s %0d %0d %0d", a.name, b.name, a.h.v, b.h.v, a.h == b.h);
        list[1] = b;
        list[2] = bump(list[1]);
        $display("2 %s %0d %0.2f", list[2].name, list[2].h.v, list[2].w);
        rename(list[2], "z");
        $display("3 %s %s", list[2].name, list[1].name);
        b.h = new(1);
        $display("4 %0d %0d %0d", a.h.v, b.h.v, list[0].h == null);
        list[0] = a;
        list[0].h.v = 99;
        $display("5 %0d", a.h.v);
        $finish(0);
    end
endmodule
