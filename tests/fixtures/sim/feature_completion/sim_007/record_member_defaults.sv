// SIM-007: default member values of records with string, real, handle and
// container leaves (SV 7.2.2). A variable declared without an initializer
// starts from the defaults of its type, including those of nested record
// types; the outermost default on a member path wins.
class Obj;
    int v;
endclass

typedef struct {
    int a = 5;
    byte b;
    logic [3:0] c = 4'hA;
    logic [3:0] u;
} fin_t;

typedef struct {
    string n = "in";
    real w = 1.5;
    fin_t f;
} in_t;

typedef struct {
    string s = "x";
    int k = 3;
    real r = 2.25;
    in_t inner;
    fin_t over = '{a: 1, b: 2, c: 3, u: 4};
    int arr[2] = '{7, 8};
    string names[2];
    Obj h = null;
    int q[$];
} r_t;

module child #(parameter int ID = 0) ();
    r_t c;
    initial begin
        c.k = c.k + ID;
        #ID $display("I %0d %s %0d", ID, c.s, c.k);
    end
endmodule

module tb;
    r_t m, m2;
    int dl = m.s.len() + m.k;
    string cat = {m.inner.n, "!"};
    child #(1) u1();
    child #(2) u2();
    initial begin
        $display("A %s %0d %0.2f | %s %0.2f %0d %0d %h %h", m.s, m.k, m.r,
                 m.inner.n, m.inner.w, m.inner.f.a, m.inner.f.b, m.inner.f.c, m.inner.f.u);
        $display("B %0d %0d %h %h | %0d %0d [%s] %0d %0d", m.over.a, m.over.b, m.over.c,
                 m.over.u, m.arr[0], m.arr[1], m.names[1], m.h == null, m.q.size());
        $display("C %0d %s", dl, cat);
        m.s = "changed";
        m.arr[0] = 0;
        $display("D %s %0d %s %0d", m.s, m.arr[0], m2.s, m2.arr[0]);
    end
    initial begin
        static r_t sb;
        for (int i = 0; i < 2; i++) begin
            automatic r_t ab;
            $display("E %s %0d | %s %0d %0.2f %s %0d %h", sb.s, sb.k, ab.s, ab.k, ab.r,
                     ab.inner.n, ab.inner.f.a, ab.inner.f.c);
            ab.s = "a"; ab.k = 0; ab.r = 0.0; ab.inner.n = "a"; ab.inner.f.a = 0;
            ab.inner.f.c = 0; sb.s = "sb"; sb.k++;
        end
        #5 $finish(0);
    end
endmodule
