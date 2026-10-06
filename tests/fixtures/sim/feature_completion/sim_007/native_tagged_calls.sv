// SIM-007-A03: tagged unions with string, real, record, class-handle and void
// members in subroutine storage: automatic and static formals, results and
// locals, output/inout copy-back through a timed task, tagged construction
// as an operand, `case matches`/`if matches` with packed, string and real
// bindings, guarded member writes and the conditional operator.
class Box;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

typedef struct { string n; int k; } rec_t;
typedef union tagged { int I; string S; real F; rec_t Rec; Box H; void None; } val_t;

module tb;
    val_t g, h, r;
    Box bx;
    logic c;

    function automatic string show(val_t v);
        case (v) matches
            tagged I .i: return $sformatf("I %0d", i);
            tagged S .s: return {"S ", s};
            tagged F .f: return $sformatf("F %.1f", f);
            tagged Rec .*: return $sformatf("R %s %0d", v.Rec.n, v.Rec.k);
            tagged H .*: return $sformatf("H %0d", v.H.v);
            tagged None: return "None";
        endcase
        return "?";
    endfunction

    function automatic val_t bump(val_t v);
        if (v matches tagged I .i) v = tagged I (i + 1);
        else if (v matches tagged S .*) v.S = {v.S, "!"};
        else if (v matches tagged Rec .*) v.Rec.k++;
        return v;
    endfunction

    function automatic val_t make(int which);
        val_t m;
        case (which)
            0: m = tagged S "s0";
            1: m = tagged F 2.5;
            2: m = tagged Rec '{"r", 3};
            3: m = tagged H bx;
            default: m = tagged None;
        endcase
        return m;
    endfunction

    task automatic later(input val_t v, output val_t o, inout val_t io);
        #1;
        o = bump(v);
        io = bump(io);
    endtask

    function int static_peek(val_t v);
        static val_t keep;
        if (v matches tagged I .*) keep = v;
        return keep.I;
    endfunction

    function automatic val_t pick(logic c, val_t a, val_t b);
        return c ? a : b;
    endfunction

    initial begin
        bx = new(9);
        for (int k = 0; k < 5; k++) $display(show(make(k)));
        g = make(2);
        g = bump(g);
        $display(show(g));
        h = tagged I 4;
        $display(show(bump(h)));
        $display("%0d %0d", static_peek(h), static_peek(tagged S "x"));
        later(make(0), g, h);
        $display("%s | %s", show(g), show(h));
        $display("%s %s", show(tagged Rec '{"p", 7}), show(tagged F 0.5));
        r = pick(1, tagged I 1, tagged S "x");
        $display(show(r));
        r = pick(0, tagged I 1, tagged S "x");
        $display(show(r));
        r = pick(1'bx, tagged S "y", tagged S "y");
        $display(show(r));
        c = 0;
        g = c ? h : r;
        $display(show(g));
        $finish(0);
    end
endmodule
