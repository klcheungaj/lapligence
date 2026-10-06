// SIM-007: queues, dynamic, associative and fixed arrays of records with
// queue and dynamic-array members (SV 7.2, 7.5, 7.8, 7.10, 11.4.5). Each
// element holds its members inside its own value; whole elements copy deeply
// to and from records, and a statement that names an element's member
// operates on it like any other container.
typedef struct {
    string name;
    int q[$];
    real d[];
    string tags[$];
} rec_t;

class bag_c;
    rec_t items[$];
    function void add(string name, int v);
        rec_t r;
        r.name = name;
        r.q.push_back(v);
        items.push_back(r);
    endfunction
    function int total();
        int t = 0;
        foreach (items[i]) t += items[i].q.sum();
        return t;
    endfunction
endclass

module tb;
    rec_t list[$];
    rec_t fixed[2];
    rec_t other[2];
    rec_t dyn[];
    rec_t by_name[string];
    rec_t r;
    int x;
    bag_c bag;

    function automatic rec_t make(string name, int n);
        rec_t m;
        m.name = name;
        for (int i = 0; i < n; i++) m.q.push_back(i);
        m.d = new[1];
        m.d[0] = real'(n) / 2.0;
        m.tags.push_back(name);
        return m;
    endfunction

    function automatic int sum(rec_t m);
        int t = 0;
        foreach (m.q[i]) t += m.q[i];
        return t;
    endfunction

    function automatic int count(rec_t xs[$], int k);
        int n;
        n = xs[k].q.size();
        return n;
    endfunction

    task automatic fill(output rec_t o, input int n);
        o.name = "out";
        for (int i = 1; i <= n; i++) o.q.push_back(i * 10);
    endtask

    initial begin
        r.name = "a";
        r.q = '{1, 2};
        list.push_back(r);
        list.push_back(make("b", 3));
        r.q.push_back(9);
        $display("1 %0d %s %0d %0d %0d", list.size(), list[0].name, list[0].q.size(),
                 list[1].q[2], r.q.size());
        r = list[1];
        $display("2 %s %0d %0d %0.1f %s", r.name, r.q.size(), r.q[2], r.d[0], r.tags[0]);
        list[0].q.push_back(5);
        list[1].q[0] = 7;
        list[1].q[1]++;
        list[0].tags.push_front("t");
        $display("3 %0d %0d %0d %0d %0d %s", list[0].q.size(), list[0].q[2], list[1].q[0],
                 list[1].q[1], sum(list[1]), list[0].tags[0]);
        x = list[1].q.pop_back();
        list[1].q.insert(0, list[0].q.size());
        $display("4 %0d %0d %0d %0d", x, list[1].q.size(), list[1].q[0], list[1].q[$]);
        r = list.pop_front();
        $display("5 %s %0d %0d", r.name, r.q.size(), list.size());
        list.insert(0, make("c", 2));
        list[0] = list[1];
        list[1].q.delete();
        $display("6 %s %0d %0d %0d", list[0].name, list[0].q.size(), list[1].q.size(),
                 list[0] == list[1]);
        fixed[1] = make("f", 4);
        fixed[0] = fixed[1];
        fixed[0].q[0] = 100;
        other = fixed;
        other[1].d[0] = 9.5;
        $display("7 %0d %0d %0d %0.1f %0.1f %0d %0d", fixed[0].q[0], fixed[1].q[0], other[0].q[0],
                 fixed[1].d[0], other[1].d[0], other == fixed, fixed[0] != fixed[1]);
        fixed = '{make("p", 1), make("q", 2)};
        $display("8 %s %0d %s %0d", fixed[0].name, sum(fixed[0]), fixed[1].name, sum(fixed[1]));
        dyn = new[2];
        dyn[1] = make("d", 3);
        fill(dyn[0], 2);
        $display("9 %s %0d %0d %0d", dyn[0].name, dyn[0].q[1], dyn[1].q.size(), count(list, 0));
        by_name["k"] = dyn[1];
        by_name["k"].q.push_back(42);
        r = by_name["k"];
        $display("10 %s %0d %0d %0d", r.name, r.q.size(), r.q[$], by_name.num());
        bag = new;
        bag.add("x", 3);
        bag.add("y", 4);
        $display("11 %0d", bag.total());
        list = '{make("m", 1), r};
        $display("12 %0d %0d %0d", list.size(), list[0].q.size(), list[1].q[$]);
        $finish(0);
    end
endmodule
