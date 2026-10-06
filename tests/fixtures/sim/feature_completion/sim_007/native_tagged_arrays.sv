// SIM-007: fixed, queue, dynamic and associative arrays of tagged unions with
// string, real, record and class-handle members (SV 7.3.2, 7.4, 7.10,
// 11.4.5, 12.6). Each element owns its tag and member storage, and every
// member access checks that element's tag.
class box_c;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

typedef struct { string s; int n; } rec_t;
typedef union tagged {
    void None;
    int I;
    string S;
    real F;
    rec_t Rec;
    box_c H;
} val_t;
typedef val_t pair_t[2];

class holder_c;
    val_t items[$];
    function void add(val_t v);
        items.push_back(v);
    endfunction
    function string first();
        return items[0].S;
    endfunction
endclass

module tb;
    val_t arr[4];
    val_t copy[4];
    val_t q[$];
    val_t d[];
    val_t a[string];
    pair_t fa, fb;
    val_t v, w;
    int i;
    holder_c h;
    box_c b;

    function automatic val_t make(int k);
        if (k > 0) return tagged I k;
        return tagged S "neg";
    endfunction

    function automatic string show(val_t x);
        case (x) matches
            tagged None: return "none";
            tagged I .n: return $sformatf("I%0d", n);
            tagged S .s: return {"S", s};
            tagged F .r: return $sformatf("F%0.1f", r);
            tagged Rec .*: return {"R", x.Rec.s};
            tagged H .*: return $sformatf("H%0d", x.H.v);
            default: return "?";
        endcase
    endfunction

    function automatic val_t pick(val_t xs[$], int k);
        return xs[k];
    endfunction

    function automatic pair_t swap(pair_t p);
        return '{p[1], p[0]};
    endfunction

    task automatic set_int(output val_t o, input int k);
        o = tagged I k;
    endtask

    task automatic bump(inout val_t o);
        if (o matches tagged I .n) o = tagged I (n + 1);
    endtask

    initial begin
        b = new(7);
        arr[0] = tagged I 5;
        arr[1] = tagged S "hi";
        arr[2] = tagged Rec '{"rec", 3};
        arr[3] = tagged H b;
        for (i = 0; i < 4; i++) $display("1 %0d %s", i, show(arr[i]));
        arr[2].Rec.n = arr[2].Rec.n + 1;
        arr[3].H.v = 8;
        arr[1].S = {arr[1].S, "!"};
        $display("2 %s %0d %0d %0d %s", arr[2].Rec.s, arr[2].Rec.n, b.v, arr[0].I + 1, arr[1].S);
        v = arr[1];
        arr[0] = v;
        v = tagged F 2.5;
        $display("3 %s %s %s", show(v), show(arr[0]), show(arr[1]));
        copy = arr;
        copy[1] = tagged None;
        $display("4 %s %s", show(arr[1]), show(copy[1]));
        if (arr[2] matches tagged Rec .*) $display("5 rec");
        if (arr[1] matches tagged S .s &&& s == "hi!") $display("6 %s", s);
        if (arr[0] matches tagged I .n) $display("7 bad %0d", n);
        else $display("7 not I");
        v = tagged I 3;
        v = tagged S "x";
        w = tagged S "x";
        arr[0] = tagged I 4;
        arr[0] = tagged S "x";
        $display("8 %0d %0d %0d %0d", v == w, arr[0] == v, arr[0] != arr[1], v == arr[3]);
        q = '{tagged I 1, tagged S "a", tagged F 0.5};
        $display("9 %0d %s %0.1f %0d", q[0].I, q[1].S, q[2].F, q.size());
        q.push_back(arr[2]);
        q.push_front(make(-1));
        q.insert(1, v);
        foreach (q[k]) $display("10 %0d %s", k, show(q[k]));
        w = q.pop_back();
        $display("11 %s %0d", show(w), q.size());
        set_int(q[1], 7);
        bump(q[1]);
        $display("12 %0d %s", q[1].I, show(pick(q, 4)));
        q.delete(0);
        $display("13 %0d %s", q.size(), show(q[0]));
        d = new[3];
        d[1] = make(9);
        $display("14 %s %s %0d", show(d[0]), show(d[1]), d[0] == d[2]);
        a["x"] = tagged S "ax";
        a["y"] = q[0];
        $display("15 %s %s %s %0d", show(a["x"]), a["x"].S, show(a["y"]), a.num());
        i = 3;
        if (q[i] matches tagged F .r &&& r > 0.25) $display("16 %0.2f", r);
        case (q[2]) matches
            tagged I .n: $display("17 bad %0d", n);
            tagged S .s: $display("17 %s", s);
        endcase
        fa = '{tagged S "l", tagged S "r"};
        fb = swap(fa);
        $display("18 %s %s %0d %0d", fb[0].S, fb[1].S, fa == fb, fb != fa);
        foreach (fa[k]) if (fa[k] matches tagged S .s) $display("19 %0d %s", k, s);
        h = new;
        h.add(tagged S "cls");
        $display("20 %s", h.first());
        for (i = 0; i < 4; i++) arr[i] = tagged I (i * 10);
        $display("21 %0d %0d", arr[3].I, arr[1].I);
        v = tagged I 3;
        v = tagged S "x";
        fa[0] = v;
        fb[0] = tagged S "x";
        fa[1] = tagged None;
        fb[1] = tagged None;
        $display("22 %0d %0d", fa[0] == fb[0], fa == fb);
        $finish(0);
    end
endmodule
