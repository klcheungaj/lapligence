// SIM-007: equality of a nested record or a member array of a record with
// string, real, handle or container leaves compares member by member like
// a whole record (SV 11.4.5): 0 when a member differs, otherwise X when a
// member compares unknown; `===` matches X and Z exactly.
class Obj;
    int v;
endclass

typedef struct {
    string n;
    logic [3:0] q;
} in_t;

typedef struct {
    in_t inner;
    in_t other;
    int arr[2];
    string sa[2];
    in_t ia[2];
    real ra[0:1];
    int dq[$];
    Obj h;
} r_t;

typedef struct {
    int vals[1:2];
} o_t;

module tb;
    r_t a, b;
    in_t solo;
    o_t o;
    initial begin
        a.inner = '{n: "x", q: 4'd1};
        b.inner = '{n: "x", q: 4'd1};
        a.arr = '{1, 2};
        b.arr = '{1, 2};
        a.sa[1] = "s";
        b.sa[1] = "s";
        $display("A %0d %0d %0d %0d %0d", a.inner == b.inner, a.arr == b.arr, a.sa == b.sa,
                 a.ia == b.ia, a.inner != b.inner);
        b.inner.n = "y";
        b.arr[1] = 3;
        b.sa[0] = "t";
        b.ia[1] = '{"z", 4'd4};
        $display("B %0d %0d %0d %0d %0d", a.inner == b.inner, a.arr != b.arr, a.sa != b.sa,
                 a.ia == b.ia, a.ia[0] == b.ia[0]);
        solo = a.inner;
        $display("C %0d %0d %0d", solo == a.inner, b.inner == solo, a.inner != solo);
        o.vals = '{1, 2};
        $display("D %0d %0d", o.vals == a.arr, o.vals != b.arr);
        a.ra[1] = 1.5;
        b.ra[1] = 1.5;
        a.dq.push_back(1);
        b.dq.push_back(1);
        $display("E %0d %0d", a.ra == b.ra, a.dq == b.dq);
        // X in one member: == is unknown while the other members are equal,
        // and 0 once a member differs; === compares X exactly.
        a.other = '{n: "o", q: 4'b10x1};
        b.other = '{n: "o", q: 4'b10x1};
        $display("F %b %b %b %b", a.other == b.other, a.other != b.other, a.other === b.other,
                 a.other !== b.other);
        b.other.n = "p";
        $display("G %b %b", a.other == b.other, a.other === b.other);
        // A pattern with whole nested-record items reads every source before
        // writing: the two members swap.
        a = '{inner: a.other, other: a.inner, arr: a.arr, sa: a.sa, ia: a.ia, ra: a.ra,
              dq: '{}, h: null};
        $display("H %s %s %b", a.inner.n, a.other.n, a.other == solo);
        $finish(0);
    end
endmodule
