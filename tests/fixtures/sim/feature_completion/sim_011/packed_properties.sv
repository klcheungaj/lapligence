// SIM-011: packed and unpacked-record class properties are fixed-value
// storage: member, bit and part selections read and write the selected
// part inside methods, through handles and through handle chains (SV 7.2,
// 8.4, 11.5.1); an unpacked record keeps two-state members at 0 and
// four-state members at X by default (SV 6.8 Table 6-7) and takes a pattern
// initializer per object (SV 8.8).
typedef struct packed {
    logic [7:0] a;
    logic [3:0] b;
} pk_t;

typedef struct {
    int a;
    logic [3:0] b;
    byte c[2];
} rec_t;

class Node;
    pk_t pk;
    rec_t rec;
    rec_t init_rec = '{a: 5, b: 4'h6, c: '{1, 2}};
    logic [7:0] v;
    int unset_int;
    Node next;
    function void setb();
        pk.b = 4'h9;
        rec.c[1] = 8'd7;
        v[0] = 1'b1;
    endfunction
endclass

module tb;
    Node n;
    Node m;

    initial begin
        n = new;
        m = new;
        n.next = m;
        $display("%0d %b %0d %0d %0d", n.rec.a, n.rec.b, n.init_rec.a, n.init_rec.c[1], n.unset_int);
        n.pk = '{a: 8'h12, b: 4'h3};
        n.pk.a = 8'h55;
        n.v = 8'h00;
        n.v[3:0] = 4'hf;
        n.v[7] = 1'b1;
        n.next.rec.a = 42;
        n.next.rec.b = 4'ha;
        n.next.rec.c[0] = 8'd9;
        $display("%h %h %h %b", n.pk, n.pk.a, n.pk.b, n.v);
        n.setb();
        $display("%h %0d %b", n.pk, n.rec.c[1], n.v);
        $display("%0d %h %0d %0d", m.rec.a, m.rec.b, m.rec.c[0], n.rec.a);
        m.rec = n.init_rec;
        $display("%0d %h %0d %0d", m.rec.a, m.rec.b, m.rec.c[1], m.rec == n.init_rec);
        $finish;
    end
endmodule
